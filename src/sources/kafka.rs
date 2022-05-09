use anyhow::{anyhow, Context};
use rskafka::client::partition::OffsetAt;
use rskafka::client::{Client, ClientBuilder, SaslConfig};
use rskafka::record::RecordAndOffset;
use rskafka::topic::Topic;
use rustls::{OwnedTrustAnchor, RootCertStore};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

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

struct KafkaConfig {
    bootstrap_servers: Vec<String>,
    credentials: Option<(String, String)>,
}

impl KafkaConfig {
    pub fn new(fname: &str) -> anyhow::Result<Self> {
        let file = File::open(fname)?;
        let mut lines = BufReader::new(file).lines();
        Ok(KafkaConfig {
            bootstrap_servers: vec![lines
                .next()
                .ok_or_else(|| anyhow!("Missing bootstrap server info in {fname}"))??],
            credentials: Some((
                lines
                    .next()
                    .ok_or_else(|| anyhow!("Missing username in {fname}"))??,
                lines
                    .next()
                    .ok_or_else(|| anyhow!("Missing password in {fname}"))??,
            )),
        })
    }
}

impl Default for KafkaConfig {
    fn default() -> Self {
        KafkaConfig {
            bootstrap_servers: vec!["localhost:9092".to_string()],
            credentials: None,
        }
    }
}

pub struct KafkaStream<T> {
    client: Client,
    topic: Topic,
    processor: T,
}

impl<T> KafkaStream<T>
where
    T: KafkaProcessor + Send + Sync + Clone + 'static,
{
    pub async fn new(topic_name: &str, processor: T) -> anyhow::Result<Self> {
        let client = Self::init_client().await?;
        let topics = client
            .list_topics()
            .await
            .with_context(|| "Failed to list topics")?;

        let topic = topics
            .into_iter()
            .find(|t| t.name == topic_name)
            .ok_or_else(|| anyhow!("Could not find topic {topic_name}"))?;
        info!(topic = ?topic);
        Ok(KafkaStream {
            client,
            topic,
            processor,
        })
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mut handles: Vec<JoinHandle<()>> = vec![];
        for partition in self.topic.partitions.iter() {
            let partition_id = *partition;
            let topic_name = self.topic.name.clone();
            let partition_client = self
                .client
                .partition_client(topic_name.clone(), partition_id)
                .await
                .with_context(|| format!("Error creating client for partition {partition_id}"))?;
            let callback = self.processor.clone();
            let handle = tokio::spawn(async move {
                let mut offset = partition_client
                    .get_offset(OffsetAt::Earliest)
                    .await
                    .unwrap();
                loop {
                    let result = partition_client
                        .fetch_records(
                            offset,       // offset
                            1..1_000_000, // min..max bytes
                            1_000,        // max wait time
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

    async fn init_client() -> anyhow::Result<Client> {
        let fname = std::env::var("WASMFLOW_KAFKA_CONFIG");
        let cfg = match fname {
            Ok(f) => KafkaConfig::new(&f)
                .with_context(|| format!("Error reading Kafka config file {f}"))?,
            Err(..) => KafkaConfig::default(),
        };

        if let Some(creds) = cfg.credentials {
            Ok(ClientBuilder::new(cfg.bootstrap_servers)
                .tls_config(Arc::new(Self::create_tls_config()))
                .sasl_config(SaslConfig::Plain {
                    username: creds.0,
                    password: creds.1,
                })
                .build()
                .await?)
        } else {
            Ok(ClientBuilder::new(cfg.bootstrap_servers).build().await?)
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
