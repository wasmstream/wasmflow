wit_bindgen_rust::export!("record-processor.wit");
use record_processor::{FlowRecord, Status};
struct RecordProcessor;

impl record_processor::RecordProcessor for RecordProcessor {
    fn parse_record(rec: FlowRecord) -> Status {
        println!("WASM - {rec:?}");
        Status::Ok
    }
}
