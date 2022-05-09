mod s3_sink {
  #[repr(u8)]
  #[derive(Clone, Copy, PartialEq, Eq)]
  pub enum Status{
    Ok,
    Error,
  }
  impl std::fmt::Debug for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
        Status::Ok => {
          f.debug_tuple("Status::Ok").finish()
        }
        Status::Error => {
          f.debug_tuple("Status::Error").finish()
        }
      }
    }
  }
  pub fn write(key: & str,body: &[u8],) -> Status{
    unsafe {
      let vec0 = key;
      let ptr0 = vec0.as_ptr() as i32;
      let len0 = vec0.len() as i32;
      let vec1 = body;
      let ptr1 = vec1.as_ptr() as i32;
      let len1 = vec1.len() as i32;
      #[link(wasm_import_module = "s3-sink")]
      extern "C" {
        #[cfg_attr(target_arch = "wasm32", link_name = "write")]
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "s3-sink_write")]
        fn wit_import(_: i32, _: i32, _: i32, _: i32, ) -> i32;
      }
      let ret = wit_import(ptr0, len0, ptr1, len1);
      match ret {
        0 => Status::Ok,
        1 => Status::Error,
        _ => panic!("invalid enum discriminant"),
      }
    }
  }
}
