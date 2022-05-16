wit_bindgen_rust::export!("../../wit/record-processor.wit");
wit_bindgen_rust::import!("../../wit/s3-sink.wit");

use record_processor::{FlowRecord, Status};
struct RecordProcessor;

impl record_processor::RecordProcessor for RecordProcessor {
    fn process_record(rec: FlowRecord) -> Status {
        let key = rec.key.as_ref().map_or("No Key", |v| {
            std::str::from_utf8(v.as_slice()).unwrap_or("UTF8 Error")
        });
        let val = rec.value.as_ref().map_or("No Value", |v| {
            std::str::from_utf8(v.as_slice()).unwrap_or("UTF8 Error")
        });
        println!("WASM Key - {key}");
        println!("WASM Value - {val}");
        println!("WASM Partition - {}", rec.partition);
        let res = s3_sink::write(rec.partition, val.as_bytes());
        if res == s3_sink::Status::Error {
            return Status::Error;
        }
        Status::Ok
    }
}
