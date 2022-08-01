use anyhow::Context;

use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    ClientConfig,
};

use crate::conf;

pub fn create_kafka_consumer(cfg: &conf::Source) -> anyhow::Result<StreamConsumer> {
    match cfg {
        conf::Source::Kafka {
            brokers,
            topic,
            group_id,
            batch_size,
            sasl,
        } => {
            let consumer: StreamConsumer = init_client_config(brokers, group_id, *batch_size, sasl)
                .create()
                .with_context(|| "Failed to initialize Kafka StreamConsumer.")?;
            consumer
                .subscribe(&[topic])
                .with_context(|| format!("StreamConsumer failed to subscribe to topic: {topic}"))?;
            Ok(consumer)
        }
    }
}

fn init_client_config(
    brokers: &[String],
    group_id: &str,
    batch_size: i32,
    sasl: &conf::SaslConfig,
) -> ClientConfig {
    let mut cfg = ClientConfig::new();
    cfg.set("group.id", group_id)
        .set("bootstrap.servers", brokers.join(","))
        .set("batch.size", batch_size.to_string())
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false");

    if let conf::SaslConfig::Plain { username, password } = sasl {
        cfg.set("security.protocol", "sasl_ssl")
            .set("sasl.mechanisms", "PLAIN")
            .set("sasl.username", username)
            .set("sasl.password", password);
    }
    cfg
}
