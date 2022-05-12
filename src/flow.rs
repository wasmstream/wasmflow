wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use std::path::PathBuf;

use anyhow::Context;
use record_processor::{FlowRecord, RecordProcessorData};
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use futures::stream::StreamExt;

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
    ) -> Self {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_module_linking(true);
        config.async_support(true);
        let engine = Engine::new(&config).expect("Could not create a new Wasmtime engine.");
        let mut linker: Linker<FlowState> = Linker::new(&engine);
        let module = Module::from_file(&engine, filename)
            .with_context(|| format!("Could not create module: {filename:?}"))
            .unwrap();
        wasmtime_wasi::add_to_linker(&mut linker, |s| &mut s.wasi)
            .expect("Failed to add wasi linker.");
        crate::sinks::s3::s3_sink::add_to_linker(&mut linker, |s| &mut s.s3_writer)
            .expect("Failed to add s3_sink");
        Self {
            engine,
            linker,
            module,
            stream_builder,
            s3_writer,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let streams = self
            .stream_builder
            .build()
            .await
            .with_context(|| "Error creating a combined stream of partitions.")?;
        streams
            .for_each_concurrent(None, |r| async move {
                let flow_state = FlowState::new(self.s3_writer.clone());
                let mut store = Store::new(&self.engine, flow_state);
                let (wasm_rec_proc, _) = record_processor::RecordProcessor::instantiate(
                    store.as_context_mut(),
                    &self.module,
                    &mut self.linker.clone(),
                    |s| &mut s.data,
                )
                .await
                .expect("Could not create WASM instance.");
                let rec = r.expect("Could not get a record.");
                let headers: Vec<(&str, &[u8])> = rec
                    .0
                    .record
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_slice()))
                    .collect();
                let frec = FlowRecord {
                    key: rec.0.record.key.as_deref(),
                    value: rec.0.record.value.as_deref(),
                    headers: &headers,
                    offset: rec.1,
                };
                let resp = wasm_rec_proc
                    .process_record(store.as_context_mut(), frec)
                    .await
                    .expect("Error invoking WASM function.");
                info!(wasm_output = ?resp);
            })
            .await;
        Ok(())
    }
}

impl FlowState {
    pub fn new(s3_writer: S3Writer) -> Self {
        Self {
            wasi: WasiCtxBuilder::new()
                .inherit_stdio()
                .inherit_args()
                .expect("Could not initialize WASI")
                .build(),
            data: RecordProcessorData {},
            s3_writer,
        }
    }
}
