use crate::conf;
use anyhow::anyhow;
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{types::ByteStream, Client, Region};
use std::{
    ops::DerefMut,
    sync::{Arc, Mutex},
};
use tracing::error;

wit_bindgen_wasmtime::export!({ paths: ["wit/s3-sink.wit"], async: * });

const S3_BUFFER_SIZE_BYTES: usize = 4096;

#[derive(Clone)]
pub struct S3Writer {
    bucket: String,
    key_prefix: String,
    client: Client,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl S3Writer {
    pub async fn new(cfg: &conf::Sink) -> anyhow::Result<Self> {
        match cfg {
            conf::Sink::S3 {
                region,
                bucket,
                key_prefix,
            } => {
                let region_provider =
                    RegionProviderChain::first_try(Region::new(region.to_string()))
                        .or_default_provider();
                let shared_config = aws_config::from_env().region(region_provider).load().await;
                let client = Client::new(&shared_config);
                Ok(Self {
                    bucket: bucket.to_string(),
                    key_prefix: key_prefix.to_string(),
                    client,
                    buffer: Arc::new(Mutex::new(Vec::with_capacity(S3_BUFFER_SIZE_BYTES))),
                })
            }
            conf::Sink::None => Err(anyhow!("Cannot create S3Writer when sink is None")),
        }
    }
}

#[async_trait]
impl s3_sink::S3Sink for S3Writer {
    async fn write(&mut self, body: &[u8]) -> s3_sink::Status {
        let mut flush_buffer: Option<Vec<u8>> = None;
        {
            let l = self.buffer.lock();
            match l {
                Err(e) => {
                    error!(s3_sink=%e);
                    return s3_sink::Status::Error;
                }
                Ok(mut g) => {
                    let b = g.deref_mut();
                    b.extend_from_slice(body);
                    if b.len() > ((0.8 * S3_BUFFER_SIZE_BYTES as f32) as usize) {
                        flush_buffer = Some(std::mem::take(b));
                        *b = Vec::with_capacity(S3_BUFFER_SIZE_BYTES);
                    }
                }
            }
        }

        match flush_buffer {
            Some(buf) => {
                let timestamp = chrono::Local::now();
                let key = format!(
                    "{}/{}",
                    self.key_prefix,
                    timestamp.format("%Y-%m-%d-%H-%M-%S")
                );
                let resp = self
                    .client
                    .put_object()
                    .bucket(self.bucket.to_string())
                    .key(key)
                    .body(ByteStream::from(buf))
                    .send()
                    .await;
                match resp {
                    Ok(_p) => s3_sink::Status::Ok,
                    Err(e) => {
                        error!(s3_sink_error=?e);
                        s3_sink::Status::Error
                    }
                }
            }
            None => s3_sink::Status::Ok,
        }
    }
}
