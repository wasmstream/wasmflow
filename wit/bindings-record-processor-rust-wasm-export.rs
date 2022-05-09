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
    #[export_name = "process-records"]
    unsafe extern "C" fn __wit_bindgen_process_records(arg0: i32, arg1: i32) -> i32 {
        let base5 = arg0;
        let len5 = arg1;
        let mut result5 = Vec::with_capacity(len5 as usize);
        for i in 0..len5 {
            let base = base5 + i * 40;
            result5.push({
                let base4 = *((base + 24) as *const i32);
                let len4 = *((base + 28) as *const i32);
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

                FlowRecord {
                    key: match i32::from(*((base + 0) as *const u8)) {
                        0 => None,
                        1 => Some({
                            let len0 = *((base + 8) as *const i32) as usize;

                            Vec::from_raw_parts(*((base + 4) as *const i32) as *mut _, len0, len0)
                        }),
                        _ => panic!("invalid enum discriminant"),
                    },
                    value: match i32::from(*((base + 12) as *const u8)) {
                        0 => None,
                        1 => Some({
                            let len1 = *((base + 20) as *const i32) as usize;

                            Vec::from_raw_parts(*((base + 16) as *const i32) as *mut _, len1, len1)
                        }),
                        _ => panic!("invalid enum discriminant"),
                    },
                    headers: result4,
                    offset: *((base + 32) as *const i64),
                }
            });
        }
        std::alloc::dealloc(
            base5 as *mut _,
            std::alloc::Layout::from_size_align_unchecked((len5 as usize) * 40, 8),
        );
        let result = <super::RecordProcessor as RecordProcessor>::process_records(result5);
        result as i32
    }
    pub trait RecordProcessor {
        fn process_records(recs: Vec<FlowRecord>) -> Status;
    }
}
