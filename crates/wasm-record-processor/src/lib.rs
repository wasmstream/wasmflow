wit_bindgen_rust::export!("record-processor.wit");

use record_processor::{FlowRecord, Status};
struct RecordProcessor;

impl record_processor::RecordProcessor for RecordProcessor {
    fn parse_records(recs: Vec<FlowRecord>) -> Status {
        for rec in recs {
            let key = rec.key.as_ref().map_or("No Key", |v| {
                std::str::from_utf8(v.as_slice()).unwrap_or("UTF8 Error")
            });
            let val = rec.value.as_ref().map_or("No Value", |v| {
                std::str::from_utf8(v.as_slice()).unwrap_or("UTF8 Error")
            });
            println!("WASM Key - {key}");
            println!("WASM Value - {val}");
            println!("WASM Offset - {}", rec.offset);
            for hdr in rec.headers {
                let v: i64 = integer_encoding::VarInt::decode_var(&hdr.1[..])
                    .unwrap_or((0, 4))
                    .0;
                println!("WASM Header - {}:{}", hdr.0, v);
            }
        }
        Status::Ok
    }
}
