wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use std::{path::PathBuf, task::Poll};

use anyhow::{anyhow, Context};
use pin_project::pin_project;
use record_processor::{FlowRecord, RecordProcessorData};
use rskafka::{client::consumer::StreamConsumer, record::Record};
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use futures::{stream::SelectAll, FutureExt, Stream, TryStreamExt};

use crate::{sinks::s3::S3Writer, sources::kafka::KafkaStreamBuilder};

use self::record_processor::Status;

#[derive(Clone)]
pub struct FlowContext {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
    pub s3_writer: S3Writer,
}

pub struct FlowProcessor {
    pub stream_builder: KafkaStreamBuilder,
    pub flow_context: FlowContext,
}

pub struct FlowState {
    pub wasi: WasiCtx,
    pub data: RecordProcessorData,
    pub s3_writer: S3Writer,
}

#[pin_project]
pub struct FlowPartitionStream {
    #[pin]
    inner: StreamConsumer,
    topic: String,
    partition_id: i32,
    flow_context: FlowContext,
}

impl Stream for FlowPartitionStream {
    type Item = anyhow::Result<Status>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let p = this.inner.poll_next(cx);
        match p {
            Poll::Pending => Poll::Pending,
            Poll::Ready(t) => match t {
                None => Poll::Ready(None),
                Some(r) => match r {
                    Ok(v) => {
                        let mut wasm_fut = Box::pin(FlowProcessor::process_record(
                            this.flow_context,
                            this.topic,
                            *this.partition_id,
                            v.0.record,
                            v.0.offset,
                        ));
                        let wasm_status = futures::ready!(wasm_fut.poll_unpin(cx));
                        Poll::Ready(Some(wasm_status))
                    }
                    Err(e) => Poll::Ready(Some(Err(anyhow!(e)))),
                },
            },
        }
    }
}

impl FlowProcessor {
    pub fn new(
        filename: &PathBuf,
        stream_builder: KafkaStreamBuilder,
        s3_writer: S3Writer,
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
        crate::sinks::s3::s3_sink::add_to_linker(&mut linker, |s| &mut s.s3_writer)
            .with_context(|| "Failed to add s3_sink")?;
        let flow_context = FlowContext {
            engine,
            linker,
            module,
            s3_writer,
        };
        Ok(Self {
            stream_builder,
            flow_context,
        })
    }

    async fn process_record(
        fctx: &mut FlowContext,
        topic: &str,
        partition_id: i32,
        rec: Record,
        offset: i64,
    ) -> anyhow::Result<Status> {
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
            partition: partition_id,
            offset,
            timestamp,
        };
        let flow_state = FlowState::new(fctx.s3_writer.clone())
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

        let streams: Vec<FlowPartitionStream> = kafka_streams
            .into_iter()
            .map(|s| FlowPartitionStream {
                inner: s.1,
                topic: self.stream_builder.topic_name().to_string(),
                partition_id: s.0,
                flow_context: self.flow_context.clone(),
            })
            .collect();

        let streams: SelectAll<FlowPartitionStream> = futures::stream::select_all(streams);

        streams
            .try_for_each_concurrent(None, |status| async move {
                info!(wasm_status=?status);
                Ok(())
            })
            .await?;

        Ok(())
    }
}

impl FlowState {
    pub fn new(s3_writer: S3Writer) -> anyhow::Result<Self> {
        Ok(Self {
            wasi: WasiCtxBuilder::new()
                .inherit_stdio()
                .inherit_args()
                .with_context(|| "Could not initialize WASI")?
                .build(),
            data: RecordProcessorData {},
            s3_writer,
        })
    }
}
