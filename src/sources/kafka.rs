use anyhow::{anyhow, Context};
use rskafka::client::{Client, ClientBuilder, SaslConfig};
use rustls::{OwnedTrustAnchor, RootCertStore};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

struct KafkaConfig {
    bootstrap_servers: Vec<String>,
    credentials: Option<(String, String)>,
}

fn read_config(fname: &str) -> anyhow::Result<KafkaConfig> {
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

fn init_default_config() -> KafkaConfig {
    KafkaConfig {
        bootstrap_servers: vec!["localhost:9092".to_string()],
        credentials: None,
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

pub async fn init_client() -> anyhow::Result<Arc<Client>> {
    let fname = std::env::var("WASMFLOW_KAFKA_CONFIG");
    let cfg = match fname {
        Ok(f) => read_config(&f).with_context(|| format!("Error reading Kafka config file {f}"))?,
        Err(..) => init_default_config(),
    };

    if let Some(creds) = cfg.credentials {
        Ok(Arc::new(
            ClientBuilder::new(cfg.bootstrap_servers)
                .tls_config(Arc::new(create_tls_config()))
                .sasl_config(SaslConfig::Plain {
                    username: creds.0,
                    password: creds.1,
                })
                .build()
                .await?,
        ))
    } else {
        Ok(Arc::new(
            ClientBuilder::new(cfg.bootstrap_servers).build().await?,
        ))
    }
}
