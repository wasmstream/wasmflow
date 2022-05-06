wit_bindgen_wasmtime::import!("./wit/record-processor.wit");
use anyhow::Context;
pub use record_processor::{FlowRecord, RecordProcessor, RecordProcessorData};
use rskafka::record::RecordAndOffset;
use tracing::info;
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

use crate::sources::kafka::KafkaProcessor;

#[derive(Clone)]
pub struct FlowProcessor {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
}

#[derive(Default)]
pub struct FlowState {
    pub wasi: Option<WasiCtx>,
    pub data: RecordProcessorData,
}

impl FlowProcessor {
    pub fn new(filename: &str) -> Self {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_module_linking(true);
        let engine = Engine::new(&config).expect("Could not create a new Wasmtime engine.");
        let mut linker: Linker<FlowState> = Linker::new(&engine);
        let module = Module::from_file(&engine, filename)
            .with_context(|| format!("Could not create module: {filename}"))
            .unwrap();
        wasmtime_wasi::add_to_linker(&mut linker, |s| s.wasi.as_mut().unwrap())
            .expect("Failed to add wasi linker.");
        Self {
            engine,
            linker,
            module,
        }
    }
}

impl KafkaProcessor for FlowProcessor {
    fn process_records(
        &self,
        _topic: &str,
        _partition_id: i32,
        recs: Vec<RecordAndOffset>,
        _high_watermark: i64,
    ) {
        let flow_state = FlowState::new();
        let mut store = Store::new(&self.engine, flow_state);
        let (wasm_rec_proc, _) = record_processor::RecordProcessor::instantiate(
            store.as_context_mut(),
            &self.module,
            &mut self.linker.clone(),
            |s| &mut s.data,
        )
        .expect("Could not create WASM instance.");

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
            .parse_records(store.as_context_mut(), &frecs)
            .expect("Error parsing record.");
        info!(wasm_output = ?resp);
    }
}

impl FlowState {
    pub fn new() -> Self {
        Self {
            wasi: Some(
                WasiCtxBuilder::new()
                    .inherit_stdio()
                    .inherit_args()
                    .expect("Could not initialize WASI")
                    .build(),
            ),
            data: RecordProcessorData {},
        }
    }
}
