use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{types::ByteStream, Client, Region};

wit_bindgen_wasmtime::export!({ paths: ["wit/s3-sink.wit"], async: * });

#[derive(Clone)]
pub struct S3Writer {
    bucket: String,
    client: Client,
}

impl S3Writer {
    pub async fn new(bucket: &str, region: &str) -> anyhow::Result<Self> {
        let region_provider =
            RegionProviderChain::first_try(Region::new(region.to_string())).or_default_provider();
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = Client::new(&shared_config);
        Ok(Self {
            bucket: bucket.to_string(),
            client,
        })
    }
}

#[async_trait]
impl s3_sink::S3Sink for S3Writer {
    async fn write(&mut self, key: &str, body: &[u8]) -> s3_sink::Status {
        /*         unsafe {
            let v = Vec::from_raw_parts(body.as_ptr() as *mut _, body.len(), body.len());
            let _resp = self
                .client
                .put_object()
                .bucket(self.bucket.to_string())
                .key(key)
                .body(ByteStream::from(v))
                .send()
                .await
                .unwrap();
        } */
        let _resp = self
            .client
            .put_object()
            .bucket(self.bucket.to_string())
            .key(key)
            .body(ByteStream::from(bytes::Bytes::copy_from_slice(body)))
            .send()
            .await
            .unwrap();
        s3_sink::Status::Ok
    }
}
