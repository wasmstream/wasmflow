wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use std::path::PathBuf;

use anyhow::Context;
use futures::TryStreamExt;
use opentelemetry::{metrics::Meter, KeyValue};
use rdkafka::{
    consumer::StreamConsumer,
    message::{BorrowedMessage, Headers},
    Message,
};
use record_processor::{FlowRecord, RecordProcessorData};
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use crate::sinks::s3::BufferedS3Sink;

use self::record_processor::Status;

#[derive(Clone)]
pub struct FlowContext {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
    pub s3_sink: BufferedS3Sink,
}

pub struct FlowProcessor {
    meter: Meter,
    pub kafka_consumer: StreamConsumer,
    pub flow_context: FlowContext,
}

pub struct FlowState {
    pub wasi: WasiCtx,
    pub data: RecordProcessorData,
    pub s3_sink: BufferedS3Sink,
}

impl FlowProcessor {
    pub fn new(
        filename: &PathBuf,
        meter: Meter,
        kafka_consumer: StreamConsumer,
        s3_sink: BufferedS3Sink,
    ) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
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
            meter,
            kafka_consumer,
            flow_context,
        })
    }

    async fn process_msg(fctx: &FlowContext, msg: &BorrowedMessage<'_>) -> anyhow::Result<Status> {
        let mut headers: Vec<(&str, &[u8])> = Vec::new();
        if let Some(hdrs) = msg.headers() {
            for idx in 0..hdrs.count() {
                if let Some((k, v)) = hdrs.get(idx) {
                    headers.push((k, v));
                }
            }
        }
        let frec = FlowRecord {
            key: msg.key(),
            value: msg.payload(),
            headers: &headers,
            topic: msg.topic(),
            partition: msg.partition(),
            offset: msg.offset(),
            timestamp: msg.timestamp().to_millis().unwrap_or(-1),
        };
        let mut fctx = fctx.clone();
        let flow_state =
            FlowState::new(fctx.s3_sink).with_context(|| "Error initializing flow state")?;
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
        let record_counter = &self
            .meter
            .u64_counter("records-processed")
            .with_description("Kafka records processed by topic and partition_id")
            .with_unit(opentelemetry::metrics::Unit::new("count"))
            .init();
        let fctx = &self.flow_context;
        self.kafka_consumer
            .stream()
            .try_for_each_concurrent(None, |msg| async move {
                let kv: [KeyValue; 2] = [
                    KeyValue::new("topic", msg.topic().to_string()),
                    KeyValue::new("partition_id", msg.partition() as i64),
                ];
                record_counter.add(1, &kv);
                let wasm_status = FlowProcessor::process_msg(fctx, &msg).await;
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
                //.inherit_args()
                // .with_context(|| "Could not initialize WASI")?
                .build(),
            data: RecordProcessorData {},
            s3_sink: s3_writer,
        })
    }
}
