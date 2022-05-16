wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use std::{path::PathBuf, pin::Pin};

use anyhow::{anyhow, Context};
use record_processor::{FlowRecord, RecordProcessorData};
use rskafka::record::Record;
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use futures::{stream::SelectAll, Stream, StreamExt, TryStreamExt};

use crate::{sinks::s3::BufferedS3Sink, sources::kafka::KafkaStreamBuilder};

use self::record_processor::Status;

#[derive(Clone)]
pub struct FlowContext {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
    pub s3_sink: BufferedS3Sink,
}

pub struct FlowProcessor {
    pub stream_builder: KafkaStreamBuilder,
    pub flow_context: FlowContext,
}

pub struct FlowState {
    pub wasi: WasiCtx,
    pub data: RecordProcessorData,
    pub s3_sink: BufferedS3Sink,
}

pub struct FlowStreamRecord {
    pub record: Record,
    pub offset: i64,
    pub partition_id: i32,
}

impl FlowProcessor {
    pub fn new(
        filename: &PathBuf,
        stream_builder: KafkaStreamBuilder,
        s3_sink: BufferedS3Sink,
    ) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_module_linking(true);
        config.async_support(true);
        let engine =
            Engine::new(&config).with_context(|| "Could not create a new Wasmtime engine.")?;
        let mut linker: Linker<FlowState> = Linker::new(&engine);
        let module = Module::from_file(&engine, filename)
            .with_context(|| format!("Could not create module: {filename:?}"))?;
        wasmtime_wasi::add_to_linker(&mut linker, |s| &mut s.wasi)
            .with_context(|| "Failed to add wasi linker.")?;
        crate::sinks::s3::s3_sink::add_to_linker(&mut linker, |s| &mut s.s3_sink)
            .with_context(|| "Failed to add s3_sink")?;
        let flow_context = FlowContext {
            engine,
            linker,
            module,
            s3_sink,
        };
        Ok(Self {
            stream_builder,
            flow_context,
        })
    }

    async fn process_record(
        fctx: &mut FlowContext,
        topic: &str,
        frec: FlowStreamRecord,
    ) -> anyhow::Result<Status> {
        let rec = frec.record;
        let timestamp = rec.timestamp.unix_timestamp();
        let headers: Vec<(&str, &[u8])> = rec
            .headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let frec = FlowRecord {
            key: rec.key.as_deref(),
            value: rec.value.as_deref(),
            headers: &headers,
            topic,
            partition: frec.partition_id,
            offset: frec.offset,
            timestamp,
        };
        let flow_state = FlowState::new(fctx.s3_sink.clone())
            .with_context(|| "Error initializing flow state")?;
        let mut store = Store::new(&fctx.engine, flow_state);
        let (record_processor, _) = record_processor::RecordProcessor::instantiate(
            store.as_context_mut(),
            &fctx.module,
            &mut fctx.linker,
            |s| &mut s.data,
        )
        .await
        .with_context(|| "Could not create WASM instance.")?;
        let status = record_processor
            .process_record(store.as_context_mut(), frec)
            .await
            .with_context(|| "Error invoking WASM function.")?;
        Ok(status)
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let kafka_streams = self
            .stream_builder
            .build()
            .await
            .with_context(|| "Error creating Kafka partition streams")?;

        let streams: Vec<Pin<Box<dyn Stream<Item = anyhow::Result<FlowStreamRecord>> + Send>>> =
            kafka_streams
                .into_iter()
                .map(|ks| {
                    let partition_id = ks.0;
                    let kstream = ks.1;
                    let f = kstream.map(move |v| match v {
                        Ok(r) => Ok(FlowStreamRecord {
                            record: r.0.record,
                            offset: r.0.offset,
                            partition_id,
                        }),
                        Err(e) => Err(anyhow!(e)),
                    });
                    f.boxed()
                })
                .collect();

        let streams: SelectAll<
            Pin<Box<dyn Stream<Item = anyhow::Result<FlowStreamRecord>> + Send>>,
        > = futures::stream::select_all(streams);

        let fctx = &self.flow_context;
        let topic_name = self.stream_builder.topic_name();
        streams
            .try_for_each_concurrent(None, |rec| async move {
                let mut f = fctx.clone();
                let wasm_status = FlowProcessor::process_record(&mut f, topic_name, rec).await;
                info!(wasm_status=?wasm_status);
                Ok(())
            })
            .await?;

        Ok(())
    }
}

impl FlowState {
    pub fn new(s3_writer: BufferedS3Sink) -> anyhow::Result<Self> {
        Ok(Self {
            wasi: WasiCtxBuilder::new()
                .inherit_stdio()
                .inherit_args()
                .with_context(|| "Could not initialize WASI")?
                .build(),
            data: RecordProcessorData {},
            s3_sink: s3_writer,
        })
    }
}
