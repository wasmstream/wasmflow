wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use std::path::PathBuf;

use anyhow::Context;
use record_processor::{FlowRecord, RecordProcessorData};
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use futures::stream::TryStreamExt;

use crate::{sinks::s3::S3Writer, sources::kafka::KafkaStreamBuilder};

pub struct FlowProcessor {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
    pub stream_builder: KafkaStreamBuilder,
    pub s3_writer: S3Writer,
}

pub struct FlowState {
    pub wasi: WasiCtx,
    pub data: RecordProcessorData,
    pub s3_writer: S3Writer,
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
        Ok(Self {
            engine,
            linker,
            module,
            stream_builder,
            s3_writer,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let streams = self
            .stream_builder
            .build()
            .await
            .with_context(|| "Error creating a combined stream of partitions.")?;
        streams
            .try_for_each_concurrent(None, |rec| async move {
                let flow_state = FlowState::new(self.s3_writer.clone())
                    .with_context(|| "Error initializing flow state")?;
                let mut store = Store::new(&self.engine, flow_state);
                let (wasm_rec_proc, _) = record_processor::RecordProcessor::instantiate(
                    store.as_context_mut(),
                    &self.module,
                    &mut self.linker.clone(),
                    |s| &mut s.data,
                )
                .await
                .with_context(|| "Could not create WASM instance.")?;
                let headers: Vec<(&str, &[u8])> = rec
                    .record
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_slice()))
                    .collect();
                let frec = FlowRecord {
                    key: rec.record.key.as_deref(),
                    value: rec.record.value.as_deref(),
                    headers: &headers,
                    topic: self.stream_builder.topic_name(),
                    partition: rec.partition,
                    offset: rec.offset,
                    timestamp: rec.timestamp,
                };
                let resp = wasm_rec_proc
                    .process_record(store.as_context_mut(), frec)
                    .await
                    .with_context(|| "Error invoking WASM function.")?;
                info!(wasm_output = ?resp);
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
