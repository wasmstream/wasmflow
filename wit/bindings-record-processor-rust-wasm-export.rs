mod record_processor {
    #[repr(u8)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Status {
        Ok,
        Error,
    }
    impl std::fmt::Debug for Status {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Status::Ok => f.debug_tuple("Status::Ok").finish(),
                Status::Error => f.debug_tuple("Status::Error").finish(),
            }
        }
    }
    #[derive(Clone)]
    pub struct FlowRecord {
        pub key: Option<Vec<u8>>,
        pub value: Option<Vec<u8>>,
        pub headers: Vec<(String, Vec<u8>)>,
        pub offset: i64,
    }
    impl std::fmt::Debug for FlowRecord {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FlowRecord")
                .field("key", &self.key)
                .field("value", &self.value)
                .field("headers", &self.headers)
                .field("offset", &self.offset)
                .finish()
        }
    }
    #[export_name = "process-record"]
    unsafe extern "C" fn __wit_bindgen_process_record(
        arg0: i32,
        arg1: i32,
        arg2: i32,
        arg3: i32,
        arg4: i32,
        arg5: i32,
        arg6: i32,
        arg7: i32,
        arg8: i64,
    ) -> i32 {
        let base4 = arg6;
        let len4 = arg7;
        let mut result4 = Vec::with_capacity(len4 as usize);
        for i in 0..len4 {
            let base = base4 + i * 16;
            result4.push({
                let len2 = *((base + 4) as *const i32) as usize;
                let len3 = *((base + 12) as *const i32) as usize;

                (
                    String::from_utf8(Vec::from_raw_parts(
                        *((base + 0) as *const i32) as *mut _,
                        len2,
                        len2,
                    ))
                    .unwrap(),
                    Vec::from_raw_parts(*((base + 8) as *const i32) as *mut _, len3, len3),
                )
            });
        }
        std::alloc::dealloc(
            base4 as *mut _,
            std::alloc::Layout::from_size_align_unchecked((len4 as usize) * 16, 4),
        );
        let result = <super::RecordProcessor as RecordProcessor>::process_record(FlowRecord {
            key: match arg0 {
                0 => None,
                1 => Some({
                    let len0 = arg2 as usize;

                    Vec::from_raw_parts(arg1 as *mut _, len0, len0)
                }),
                _ => panic!("invalid enum discriminant"),
            },
            value: match arg3 {
                0 => None,
                1 => Some({
                    let len1 = arg5 as usize;

                    Vec::from_raw_parts(arg4 as *mut _, len1, len1)
                }),
                _ => panic!("invalid enum discriminant"),
            },
            headers: result4,
            offset: arg8,
        });
        result as i32
    }
    pub trait RecordProcessor {
        fn process_record(rec: FlowRecord) -> Status;
    }
}
