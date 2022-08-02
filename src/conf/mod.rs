use anyhow::{Context, Result};
use educe::Educe;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Formatter};
use std::fs;
use std::path::PathBuf;
use tracing::info;

fn fmt_redact(_s: &str, f: &mut Formatter) -> fmt::Result {
    f.write_str("** Redacted **")
}

#[derive(Educe, Serialize, Deserialize)]
#[educe(Debug)]
pub enum SaslConfig {
    None,
    Plain {
        #[educe(Debug(method = "fmt_redact"))]
        username: String,
        #[educe(Debug(method = "fmt_redact"))]
        password: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Source {
    Kafka {
        brokers: Vec<String>,
        group_id: String,
        topic: String,
        batch_size: i32,
        sasl: SaslConfig,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Sink {
    None,
    S3 {
        region: String,
        bucket: String,
        key_prefix: String,
        file_size: u16,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Processor {
    pub module_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowConfig {
    pub sources: Vec<Source>,
    pub sinks: Vec<Sink>,
    pub processors: Vec<Processor>,
}

pub fn read_config() -> Result<FlowConfig> {
    let fname = std::env::var("WASMFLOW_CONFIG")
        .with_context(|| "Error reading WASMFLOW_CONFIG var. Did you remember to set it?")?;
    read_config_file(&fname)
}

pub fn read_config_file(fname: &str) -> Result<FlowConfig> {
    let yaml_str =
        fs::read_to_string(fname).with_context(|| format!("Error reading conf file {fname}"))?;
    let conf: FlowConfig = serde_yaml::from_str(&yaml_str)
        .with_context(|| format!("Error parsing YAML conf file {fname}"))?;
    info!(conf=?conf);
    Ok(conf)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_read_config() {
        let cfg = read_config_file("./src/conf/wasmflow.yml").unwrap();
        assert_eq!(cfg.sources.len(), 1);
        match &cfg.sources[0] {
            Source::Kafka {
                brokers,
                group_id,
                topic,
                batch_size,
                sasl,
            } => {
                assert_eq!(brokers.len(), 1);
                assert_eq!(brokers[0], "my-broker.confluent.cloud:9092");
                assert_eq!(group_id, "wasmflow-group");
                assert_eq!(topic, "my-topic");
                assert_eq!(*batch_size, 1000000);
                assert!(matches!(
                    sasl,
                    SaslConfig::Plain {
                        username: _,
                        password: _
                    }
                ));
            }
        }
        assert_eq!(cfg.sinks.len(), 1);
        match &cfg.sinks[0] {
            Sink::S3 {
                region,
                bucket,
                key_prefix,
                file_size,
            } => {
                assert_eq!(region, "us-east-1");
                assert_eq!(bucket, "wasmtime-sink");
                assert_eq!(key_prefix, "my-stream");
                assert_eq!(*file_size, 4096);
            }
            _ => {
                panic!("Incorrect sink config");
            }
        }
        assert_eq!(cfg.processors.len(), 1);
    }
}
