wit_bindgen_rust::export!("../../wit/record-processor.wit");

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
        println!("WASM Topic - {}", rec.topic);
        println!("WASM Partition - {}", rec.partition);
        println!("WASM Offset - {}", rec.offset);
        println!("WASM Timestamp - {}", rec.timestamp);
        println!("WASM Key - {key}");
        println!("WASM Value - {val}");
        for hdr in rec.headers {
            let v: i64 = integer_encoding::VarInt::decode_var(&hdr.1[..])
                .unwrap_or((0, 4))
                .0;
            println!("WASM Header - {}:{}", hdr.0, v);
        }
        Status::Ok
    }
}
