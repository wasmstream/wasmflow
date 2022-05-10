use anyhow::{anyhow, Context};
use rskafka::client::partition::OffsetAt;
use rskafka::client::{Client, ClientBuilder, SaslConfig};
use rskafka::record::RecordAndOffset;
use rskafka::topic::Topic;
use rustls::{OwnedTrustAnchor, RootCertStore};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

use crate::conf;

#[async_trait::async_trait]
pub trait KafkaProcessor {
    async fn process_records(
        &self,
        topic: &str,
        partition_id: i32,
        recs: Vec<RecordAndOffset>,
        high_watermark: i64,
    ) -> anyhow::Result<()>;
}

pub struct KafkaStream<T> {
    client: Client,
    topic: Topic,
    batch_size: i32,
    offset: OffsetAt,
    processor: T,
}

impl<T> KafkaStream<T>
where
    T: KafkaProcessor + Send + Sync + Clone + 'static,
{
    pub async fn new(cfg: &conf::Source, processor: T) -> anyhow::Result<Self> {
        match cfg {
            conf::Source::Kafka {
                brokers,
                topic,
                batch_size,
                offset,
                sasl,
            } => {
                let client = Self::init_client(brokers, sasl).await?;
                let topics = client
                    .list_topics()
                    .await
                    .with_context(|| "Failed to list topics")?;

                let matched_topic = topics
                    .into_iter()
                    .find(|t| t.name == *topic)
                    .ok_or_else(|| anyhow!("Could not find topic {topic}"))?;
                info!(topic = ?topic);
                Ok(KafkaStream {
                    client,
                    topic: matched_topic,
                    batch_size: *batch_size,
                    offset: *offset,
                    processor,
                })
            }
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mut handles: Vec<JoinHandle<()>> = vec![];
        for partition in self.topic.partitions.iter() {
            let partition_id = *partition;
            let topic_name = self.topic.name.clone();
            let offset_type = self.offset;
            let batch_size = self.batch_size;
            let partition_client = self
                .client
                .partition_client(topic_name.clone(), partition_id)
                .await
                .with_context(|| format!("Error creating client for partition {partition_id}"))?;
            let callback = self.processor.clone();
            let handle = tokio::spawn(async move {
                let mut offset = partition_client.get_offset(offset_type).await.unwrap();
                loop {
                    let result = partition_client
                        .fetch_records(
                            offset,        // offset
                            1..batch_size, // min..max bytes
                            1_000,         // max wait time
                        )
                        .await
                        .with_context(|| {
                            format!("Error fetching records from partition {partition_id}.")
                        })
                        .unwrap();
                    if !result.0.is_empty() {
                        callback
                            .process_records(&topic_name, partition_id, result.0, result.1)
                            .await
                            .expect("Error with Kafka processor records callback.");
                    } else {
                        // Sleep for 10s
                        tokio::time::sleep(tokio::time::Duration::from_millis(10000)).await;
                    }
                    offset = result.1;
                }
            });
            handles.push(handle);
        }
        for handle in handles {
            handle.await.unwrap();
        }
        Ok(())
    }

    async fn init_client(brokers: &[String], sasl: &conf::SaslConfig) -> anyhow::Result<Client> {
        match sasl {
            conf::SaslConfig::None => Ok(ClientBuilder::new(brokers.to_vec()).build().await?),
            conf::SaslConfig::Plain { username, password } => {
                Ok(ClientBuilder::new(brokers.to_vec())
                    .tls_config(Arc::new(Self::create_tls_config()))
                    .sasl_config(SaslConfig::Plain {
                        username: username.to_string(),
                        password: password.to_string(),
                    })
                    .build()
                    .await?)
            }
        }
    }

    fn create_tls_config() -> rustls::ClientConfig {
        let mut root_store = RootCertStore::empty();
        root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    }
}
