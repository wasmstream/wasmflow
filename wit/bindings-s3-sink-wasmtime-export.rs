#[allow(clippy::all)]
pub mod s3_sink {
    #[allow(unused_imports)]
    use wit_bindgen_wasmtime::{anyhow, wasmtime};
    #[repr(u8)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Status {
        Ok,
        Error,
    }
    impl core::fmt::Debug for Status {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Status::Ok => f.debug_tuple("Status::Ok").finish(),
                Status::Error => f.debug_tuple("Status::Error").finish(),
            }
        }
    }
    pub trait S3Sink: Sized {
        fn write(&mut self, partition: i32, body: &[u8]) -> Status;
    }

    pub fn add_to_linker<T, U>(
        linker: &mut wasmtime::Linker<T>,
        get: impl Fn(&mut T) -> &mut U + Send + Sync + Copy + 'static,
    ) -> anyhow::Result<()>
    where
        U: S3Sink,
    {
        use wit_bindgen_wasmtime::rt::get_memory;
        linker.func_wrap(
            "s3-sink",
            "write",
            move |mut caller: wasmtime::Caller<'_, T>, arg0: i32, arg1: i32, arg2: i32| {
                let memory = &get_memory(&mut caller, "memory")?;
                let (mem, data) = memory.data_and_store_mut(&mut caller);
                let mut _bc = wit_bindgen_wasmtime::BorrowChecker::new(mem);
                let host = get(data);
                let ptr0 = arg1;
                let len0 = arg2;
                let param0 = arg0;
                let param1 = _bc.slice(ptr0, len0)?;
                let result = host.write(param0, param1);
                Ok(result as i32)
            },
        )?;
        Ok(())
    }
}
