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
        process_record: wasmtime::TypedFunc<(i32, i32, i32, i32, i32, i32, i32, i32, i64), (i32,)>,
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
            let process_record = instance
                .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i32, i64), (i32,), _>(
                    &mut store,
                    "process-record",
                )?;
            Ok(RecordProcessor {
                canonical_abi_realloc,
                memory,
                process_record,
                get_state: Box::new(get_state),
            })
        }
        pub fn process_record(
            &self,
            mut caller: impl wasmtime::AsContextMut<Data = T>,
            rec: FlowRecord<'_>,
        ) -> Result<Status, wasmtime::Trap> {
            let func_canonical_abi_realloc = &self.canonical_abi_realloc;
            let memory = &self.memory;
            let FlowRecord {
                key: key0,
                value: value0,
                headers: headers0,
                offset: offset0,
            } = rec;
            let (result2_0, result2_1, result2_2) = match key0 {
                None => (0i32, 0i32, 0i32),
                Some(e) => {
                    let vec1 = e;
                    let ptr1 = func_canonical_abi_realloc
                        .call(&mut caller, (0, 0, 1, (vec1.len() as i32) * 1))?;
                    memory.data_mut(&mut caller).store_many(ptr1, &vec1)?;
                    (1i32, ptr1, vec1.len() as i32)
                }
            };
            let (result4_0, result4_1, result4_2) = match value0 {
                None => (0i32, 0i32, 0i32),
                Some(e) => {
                    let vec3 = e;
                    let ptr3 = func_canonical_abi_realloc
                        .call(&mut caller, (0, 0, 1, (vec3.len() as i32) * 1))?;
                    memory.data_mut(&mut caller).store_many(ptr3, &vec3)?;
                    (1i32, ptr3, vec3.len() as i32)
                }
            };
            let vec8 = headers0;
            let len8 = vec8.len() as i32;
            let result8 = func_canonical_abi_realloc.call(&mut caller, (0, 0, 4, len8 * 16))?;
            for (i, e) in vec8.into_iter().enumerate() {
                let base = result8 + (i as i32) * 16;
                {
                    let (t5_0, t5_1) = e;
                    let vec6 = t5_0;
                    let ptr6 = func_canonical_abi_realloc
                        .call(&mut caller, (0, 0, 1, vec6.len() as i32))?;
                    memory
                        .data_mut(&mut caller)
                        .store_many(ptr6, vec6.as_bytes())?;
                    memory.data_mut(&mut caller).store(
                        base + 4,
                        wit_bindgen_wasmtime::rt::as_i32(vec6.len() as i32),
                    )?;
                    memory
                        .data_mut(&mut caller)
                        .store(base + 0, wit_bindgen_wasmtime::rt::as_i32(ptr6))?;
                    let vec7 = t5_1;
                    let ptr7 = func_canonical_abi_realloc
                        .call(&mut caller, (0, 0, 1, (vec7.len() as i32) * 1))?;
                    memory.data_mut(&mut caller).store_many(ptr7, &vec7)?;
                    memory.data_mut(&mut caller).store(
                        base + 12,
                        wit_bindgen_wasmtime::rt::as_i32(vec7.len() as i32),
                    )?;
                    memory
                        .data_mut(&mut caller)
                        .store(base + 8, wit_bindgen_wasmtime::rt::as_i32(ptr7))?;
                }
            }
            let (result9_0,) = self.process_record.call(
                &mut caller,
                (
                    result2_0,
                    result2_1,
                    result2_2,
                    result4_0,
                    result4_1,
                    result4_2,
                    result8,
                    len8,
                    wit_bindgen_wasmtime::rt::as_i64(offset0),
                ),
            )?;
            Ok(match result9_0 {
                0 => Status::Ok,
                1 => Status::Error,
                _ => return Err(invalid_variant("Status")),
            })
        }
    }
    use wit_bindgen_wasmtime::rt::invalid_variant;
    use wit_bindgen_wasmtime::rt::RawMem;
}
