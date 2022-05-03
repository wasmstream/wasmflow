wit_bindgen_wasmtime::import!("crates/wasm-record-processor/record-processor.wit");
use anyhow::Context;
pub use record_processor::{FlowRecord, RecordProcessor, RecordProcessorData};
use wasi_common::WasiCtx;
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

pub struct FlowContext {
    pub engine: Engine,
    pub linker: Linker<FlowState>,
    pub module: Module,
}

#[derive(Default)]
pub struct FlowState {
    pub wasi: Option<WasiCtx>,
    pub data: RecordProcessorData,
}

impl FlowContext {
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
