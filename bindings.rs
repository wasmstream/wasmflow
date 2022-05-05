pub mod record_processor {
    #[allow(unused_imports)]
    use wit_bindgen_wasmtime::{anyhow, wasmtime};
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
    pub struct FlowRecord<'a> {
        pub key: Option<&'a [u8]>,
        pub value: Option<&'a [u8]>,
        pub headers: &'a [(&'a str, &'a [u8])],
        pub offset: i64,
    }
    impl<'a> std::fmt::Debug for FlowRecord<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FlowRecord")
                .field("key", &self.key)
                .field("value", &self.value)
                .field("headers", &self.headers)
                .field("offset", &self.offset)
                .finish()
        }
    }

    /// Auxiliary data associated with the wasm exports.
    ///
    /// This is required to be stored within the data of a
    /// `Store<T>` itself so lifting/lowering state can be managed
    /// when translating between the host and wasm.
    #[derive(Default)]
    pub struct RecordProcessorData {}
    pub struct RecordProcessor<T> {
        get_state: Box<dyn Fn(&mut T) -> &mut RecordProcessorData + Send + Sync>,
        canonical_abi_realloc: wasmtime::TypedFunc<(i32, i32, i32, i32), i32>,
        memory: wasmtime::Memory,
        parse_records: wasmtime::TypedFunc<(i32, i32), (i32,)>,
    }
    impl<T> RecordProcessor<T> {
        #[allow(unused_variables)]

        /// Adds any intrinsics, if necessary for this exported wasm
        /// functionality to the `linker` provided.
        ///
        /// The `get_state` closure is required to access the
        /// auxiliary data necessary for these wasm exports from
        /// the general store's state.
        pub fn add_to_linker(
            linker: &mut wasmtime::Linker<T>,
            get_state: impl Fn(&mut T) -> &mut RecordProcessorData + Send + Sync + Copy + 'static,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        /// Instantiates the provided `module` using the specified
        /// parameters, wrapping up the result in a structure that
        /// translates between wasm and the host.
        ///
        /// The `linker` provided will have intrinsics added to it
        /// automatically, so it's not necessary to call
        /// `add_to_linker` beforehand. This function will
        /// instantiate the `module` otherwise using `linker`, and
        /// both an instance of this structure and the underlying
        /// `wasmtime::Instance` will be returned.
        ///
        /// The `get_state` parameter is used to access the
        /// auxiliary state necessary for these wasm exports from
        /// the general store state `T`.
        pub fn instantiate(
            mut store: impl wasmtime::AsContextMut<Data = T>,
            module: &wasmtime::Module,
            linker: &mut wasmtime::Linker<T>,
            get_state: impl Fn(&mut T) -> &mut RecordProcessorData + Send + Sync + Copy + 'static,
        ) -> anyhow::Result<(Self, wasmtime::Instance)> {
            Self::add_to_linker(linker, get_state)?;
            let instance = linker.instantiate(&mut store, module)?;
            Ok((Self::new(store, &instance, get_state)?, instance))
        }

        /// Low-level creation wrapper for wrapping up the exports
        /// of the `instance` provided in this structure of wasm
        /// exports.
        ///
        /// This function will extract exports from the `instance`
        /// defined within `store` and wrap them all up in the
        /// returned structure which can be used to interact with
        /// the wasm module.
        pub fn new(
            mut store: impl wasmtime::AsContextMut<Data = T>,
            instance: &wasmtime::Instance,
            get_state: impl Fn(&mut T) -> &mut RecordProcessorData + Send + Sync + Copy + 'static,
        ) -> anyhow::Result<Self> {
            let mut store = store.as_context_mut();
            let canonical_abi_realloc = instance.get_typed_func::<(i32, i32, i32, i32), i32, _>(
                &mut store,
                "canonical_abi_realloc",
            )?;
            let memory = instance
                .get_memory(&mut store, "memory")
                .ok_or_else(|| anyhow::anyhow!("`memory` export not a memory"))?;
            let parse_records =
                instance.get_typed_func::<(i32, i32), (i32,), _>(&mut store, "parse-records")?;
            Ok(RecordProcessor {
                canonical_abi_realloc,
                memory,
                parse_records,
                get_state: Box::new(get_state),
            })
        }
        pub fn parse_records(
            &self,
            mut caller: impl wasmtime::AsContextMut<Data = T>,
            recs: &[FlowRecord<'_>],
        ) -> Result<Status, wasmtime::Trap> {
            let func_canonical_abi_realloc = &self.canonical_abi_realloc;
            let memory = &self.memory;
            let vec7 = recs;
            let len7 = vec7.len() as i32;
            let result7 = func_canonical_abi_realloc.call(&mut caller, (0, 0, 8, len7 * 40))?;
            for (i, e) in vec7.into_iter().enumerate() {
                let base = result7 + (i as i32) * 40;
                {
                    let FlowRecord {
                        key: key0,
                        value: value0,
                        headers: headers0,
                        offset: offset0,
                    } = e;
                    match key0 {
                        None => {
                            memory
                                .data_mut(&mut caller)
                                .store(base + 0, wit_bindgen_wasmtime::rt::as_i32(0i32) as u8)?;
                        }
                        Some(e) => {
                            memory
                                .data_mut(&mut caller)
                                .store(base + 0, wit_bindgen_wasmtime::rt::as_i32(1i32) as u8)?;
                            let vec1 = e;
                            let ptr1 = func_canonical_abi_realloc
                                .call(&mut caller, (0, 0, 1, (vec1.len() as i32) * 1))?;
                            memory.data_mut(&mut caller).store_many(ptr1, &vec1)?;
                            memory.data_mut(&mut caller).store(
                                base + 8,
                                wit_bindgen_wasmtime::rt::as_i32(vec1.len() as i32),
                            )?;
                            memory
                                .data_mut(&mut caller)
                                .store(base + 4, wit_bindgen_wasmtime::rt::as_i32(ptr1))?;
                        }
                    };
                    match value0 {
                        None => {
                            memory
                                .data_mut(&mut caller)
                                .store(base + 12, wit_bindgen_wasmtime::rt::as_i32(0i32) as u8)?;
                        }
                        Some(e) => {
                            memory
                                .data_mut(&mut caller)
                                .store(base + 12, wit_bindgen_wasmtime::rt::as_i32(1i32) as u8)?;
                            let vec2 = e;
                            let ptr2 = func_canonical_abi_realloc
                                .call(&mut caller, (0, 0, 1, (vec2.len() as i32) * 1))?;
                            memory.data_mut(&mut caller).store_many(ptr2, &vec2)?;
                            memory.data_mut(&mut caller).store(
                                base + 20,
                                wit_bindgen_wasmtime::rt::as_i32(vec2.len() as i32),
                            )?;
                            memory
                                .data_mut(&mut caller)
                                .store(base + 16, wit_bindgen_wasmtime::rt::as_i32(ptr2))?;
                        }
                    };
                    let vec6 = headers0;
                    let len6 = vec6.len() as i32;
                    let result6 =
                        func_canonical_abi_realloc.call(&mut caller, (0, 0, 4, len6 * 16))?;
                    for (i, e) in vec6.into_iter().enumerate() {
                        let base = result6 + (i as i32) * 16;
                        {
                            let (t3_0, t3_1) = e;
                            let vec4 = t3_0;
                            let ptr4 = func_canonical_abi_realloc
                                .call(&mut caller, (0, 0, 1, vec4.len() as i32))?;
                            memory
                                .data_mut(&mut caller)
                                .store_many(ptr4, vec4.as_bytes())?;
                            memory.data_mut(&mut caller).store(
                                base + 4,
                                wit_bindgen_wasmtime::rt::as_i32(vec4.len() as i32),
                            )?;
                            memory
                                .data_mut(&mut caller)
                                .store(base + 0, wit_bindgen_wasmtime::rt::as_i32(ptr4))?;
                            let vec5 = t3_1;
                            let ptr5 = func_canonical_abi_realloc
                                .call(&mut caller, (0, 0, 1, (vec5.len() as i32) * 1))?;
                            memory.data_mut(&mut caller).store_many(ptr5, &vec5)?;
                            memory.data_mut(&mut caller).store(
                                base + 12,
                                wit_bindgen_wasmtime::rt::as_i32(vec5.len() as i32),
                            )?;
                            memory
                                .data_mut(&mut caller)
                                .store(base + 8, wit_bindgen_wasmtime::rt::as_i32(ptr5))?;
                        }
                    }
                    memory
                        .data_mut(&mut caller)
                        .store(base + 28, wit_bindgen_wasmtime::rt::as_i32(len6))?;
                    memory
                        .data_mut(&mut caller)
                        .store(base + 24, wit_bindgen_wasmtime::rt::as_i32(result6))?;
                    memory.data_mut(&mut caller).store(
                        base + 32,
                        wit_bindgen_wasmtime::rt::as_i64(wit_bindgen_wasmtime::rt::as_i64(offset0)),
                    )?;
                }
            }
            let (result8_0,) = self.parse_records.call(&mut caller, (result7, len7))?;
            Ok(match result8_0 {
                0 => Status::Ok,
                1 => Status::Error,
                _ => return Err(invalid_variant("Status")),
            })
        }
    }
    use wit_bindgen_wasmtime::rt::invalid_variant;
    use wit_bindgen_wasmtime::rt::RawMem;
}
