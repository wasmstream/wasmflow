use crate::conf;
use anyhow::anyhow;
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{types::ByteStream, Client, Region};
use std::{
    collections::BTreeMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
};
use tracing::{debug, error};

wit_bindgen_wasmtime::export!({ paths: ["wit/s3-sink.wit"], async: * });

const S3_BUFFER_SIZE_BYTES: usize = 4096;

#[derive(Clone)]
pub struct BufferedS3Sink {
    bucket: String,
    key_prefix: String,
    client: Client,
    buffer: Arc<Mutex<BTreeMap<i32, Vec<u8>>>>,
}

impl BufferedS3Sink {
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
                    buffer: Arc::new(Mutex::new(BTreeMap::new())),
                })
            }
            conf::Sink::None => Err(anyhow!("Cannot create S3Writer when sink is None")),
        }
    }
}

#[async_trait]
impl s3_sink::S3Sink for BufferedS3Sink {
    async fn write(&mut self, partition_id: i32, body: &[u8]) -> s3_sink::Status {
        let mut flush_buffer: Option<Vec<u8>> = None;
        {
            let l = self.buffer.lock();
            match l {
                Err(e) => {
                    error!(s3_sink=%e);
                    return s3_sink::Status::Error;
                }
                Ok(mut g) => {
                    let m = g.deref_mut();
                    let buf = m
                        .entry(partition_id)
                        .or_insert_with(|| Vec::with_capacity(S3_BUFFER_SIZE_BYTES));
                    buf.extend_from_slice(body);
                    if buf.len() > ((0.8 * S3_BUFFER_SIZE_BYTES as f32) as usize) {
                        flush_buffer =
                            m.insert(partition_id, Vec::with_capacity(S3_BUFFER_SIZE_BYTES));
                    }
                }
            }
        }

        match flush_buffer {
            Some(buf) => {
                let timestamp = chrono::Local::now();
                let key = format!(
                    "{}/{}/{}/{}",
                    self.key_prefix,
                    partition_id,
                    timestamp.format("%Y/%m/%d/%H/%M/%S"),
                    uuid::Uuid::new_v4()
                );
                debug!(s3_key=%key);
                let resp = self
                    .client
                    .put_object()
                    .bucket(self.bucket.to_string())
                    .key(key)
                    .body(ByteStream::from(buf))
                    .send()
                    .await;
                debug!(resp=?resp);
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
