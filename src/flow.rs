wit_bindgen_wasmtime::import!({ paths: ["./wit/record-processor.wit"], async: *});
use anyhow::Context;
pub use record_processor::{FlowRecord, RecordProcessor, RecordProcessorData};
use rskafka::record::RecordAndOffset;
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use crate::{sinks::s3::s3_sink::S3Sink, sources::kafka::KafkaProcessor};

#[derive(Clone)]
pub struct FlowProcessor<T> {
    pub engine: Engine,
    pub linker: Linker<FlowState<T>>,
    pub module: Module,
    pub s3_sink: T,
}

#[derive(Default)]
pub struct FlowState<T> {
    pub wasi: Option<WasiCtx>,
    pub data: RecordProcessorData,
    pub s3_sink: Option<T>,
}

impl<T: S3Sink> FlowProcessor<T> {
    pub fn new(filename: &str, s3_sink: T) -> Self {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_module_linking(true);
        config.async_support(true);
        let engine = Engine::new(&config).expect("Could not create a new Wasmtime engine.");
        let mut linker: Linker<FlowState<T>> = Linker::new(&engine);
        let module = Module::from_file(&engine, filename)
            .with_context(|| format!("Could not create module: {filename}"))
            .unwrap();
        wasmtime_wasi::add_to_linker(&mut linker, |s| s.wasi.as_mut().unwrap())
            .expect("Failed to add wasi linker.");
        crate::sinks::s3::s3_sink::add_to_linker(&mut linker, |s| s.s3_sink.as_mut().unwrap())
            .expect("Failed to add s3_sink");
        Self {
            engine,
            linker,
            module,
            s3_sink,
        }
    }
}

#[async_trait::async_trait]
impl<T: S3Sink + Clone + Sync> KafkaProcessor for FlowProcessor<T> {
    async fn process_records(
        &self,
        _topic: &str,
        _partition_id: i32,
        recs: Vec<RecordAndOffset>,
        _high_watermark: i64,
    ) -> anyhow::Result<()> {
        let flow_state = FlowState::new(self.s3_sink.clone());
        let mut store = Store::new(&self.engine, flow_state);
        let (wasm_rec_proc, _) = record_processor::RecordProcessor::instantiate(
            store.as_context_mut(),
            &self.module,
            &mut self.linker.clone(),
            |s| &mut s.data,
        )
        .await
        .with_context(|| "Could not create WASM instance.")?;

        let rec_headers: Vec<Vec<(&str, &[u8])>> = recs
            .iter()
            .map(|r| {
                r.record
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_slice()))
                    .collect()
            })
            .collect();

        let frecs: Vec<FlowRecord> = recs
            .iter()
            .zip(rec_headers.iter())
            .map(|(r, h)| FlowRecord {
                key: r.record.key.as_deref(),
                value: r.record.value.as_deref(),
                headers: h,
                offset: r.offset,
            })
            .collect();

        let resp = wasm_rec_proc
            .process_records(store.as_context_mut(), &frecs)
            .await
            .with_context(|| "Error invoking WASM function.")?;
        info!(wasm_output = ?resp);
        Ok(())
    }
}

impl<T: S3Sink> FlowState<T> {
    pub fn new(s3_sink: T) -> Self {
        Self {
            wasi: Some(
                WasiCtxBuilder::new()
                    .inherit_stdio()
                    .inherit_args()
                    .expect("Could not initialize WASI")
                    .build(),
            ),
            data: RecordProcessorData {},
            s3_sink: Some(s3_sink),
        }
    }
}
