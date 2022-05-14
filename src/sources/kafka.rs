use anyhow::{anyhow, Context};

use rskafka::client::consumer::{StartOffset, StreamConsumer, StreamConsumerBuilder};
use rskafka::client::partition::OffsetAt;
use rskafka::client::{Client, ClientBuilder, SaslConfig};
use rskafka::topic::Topic;
use rustls::{OwnedTrustAnchor, RootCertStore};
use std::sync::Arc;
use tracing::info;

use crate::conf;

pub struct KafkaStreamBuilder {
    client: Client,
    topic: Topic,
    batch_size: i32,
    offset: OffsetAt,
}

impl KafkaStreamBuilder {
    pub async fn new(cfg: &conf::Source) -> anyhow::Result<Self> {
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
                info!(topic = ?matched_topic);

                Ok(KafkaStreamBuilder {
                    client,
                    topic: matched_topic,
                    batch_size: *batch_size,
                    offset: *offset,
                })
            }
        }
    }

    pub async fn build(&self) -> anyhow::Result<Vec<(i32, StreamConsumer)>> {
        let mut streams: Vec<(i32, StreamConsumer)> = vec![];
        for partition_id in self.topic.partitions.iter() {
            let partition_client = self
                .client
                .partition_client(self.topic.name.to_string(), *partition_id)
                .with_context(|| format!("Error creating client for partition {partition_id}"))?;
            let start_offset = match self.offset {
                OffsetAt::Earliest => StartOffset::Earliest,
                OffsetAt::Latest => StartOffset::Latest,
            };
            streams.push((
                *partition_id,
                StreamConsumerBuilder::new(Arc::new(partition_client), start_offset)
                    .with_max_batch_size(self.batch_size)
                    .build(),
            ));
        }
        Ok(streams)
    }

    pub fn topic_name(&self) -> &str {
        &self.topic.name
    }

    async fn init_client(brokers: &[String], sasl: &conf::SaslConfig) -> anyhow::Result<Client> {
        let mut c = ClientBuilder::new(brokers.to_vec());
        if let conf::SaslConfig::Plain { username, password } = sasl {
            c = c
                .tls_config(Arc::new(Self::create_tls_config()))
                .sasl_config(SaslConfig::Plain {
                    username: username.to_string(),
                    password: password.to_string(),
                });
        }
        c.build()
            .await
            .with_context(|| "Error initializing Kafka client.")
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
