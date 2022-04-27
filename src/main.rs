use anyhow::Context;
use rskafka::{client::Client, record::RecordAndOffset};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use wasmflow::kafka;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client = kafka::init_kafka_client()
        .await
        .with_context(|| "Failed to initialize Kafka client.")?;

    let topics = client
        .list_topics()
        .await
        .with_context(|| "Failed to list topics")?;

    if !topics.is_empty() {
        let mut handles: Vec<JoinHandle<()>> = vec![];
        for partition in topics[0].partitions.iter() {
            let client_clone = client.clone();
            let partition_id = *partition;
            let topic_name = topics[0].name.clone();
            let handle = tokio::spawn(async move {
                let _res = process_partition(client_clone, &topic_name, partition_id).await;
            });
            handles.push(handle);
        }
        for handle in handles {
            handle.await.unwrap();
        }
    } else {
        warn!("No topics found. Shutting down.");
    }
    Ok(())
}

async fn process_partition(
    client: Arc<Client>,
    topic_name: &str,
    partition_id: i32,
) -> anyhow::Result<()> {
    let partition_client = client
        .partition_client(topic_name, partition_id)
        .await
        .with_context(|| format!("Error creating client for partition {partition_id}"))?;
    loop {
        let result = partition_client
            .fetch_records(
                0,            // offset
                1..1_000_000, // min..max bytes
                1_000,        // max wait time
            )
            .await
            .with_context(|| format!("Error fetching records from partition {partition_id}."))?;
        process_result(result.0, result.1, partition_id);
    }
}

fn process_result(recs: Vec<RecordAndOffset>, _high_watermark: i64, partition_id: i32) {
    for rec in recs {
        info!(partition=partition_id, rec=?rec);
    }
}
