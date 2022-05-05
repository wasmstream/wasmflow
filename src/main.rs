use anyhow::Context;
use rskafka::{
    client::{partition::OffsetAt, Client},
    record::RecordAndOffset,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use wasmflow::flow::{FlowContext, FlowRecord, FlowState};
use wasmtime::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let fctx = Arc::new(wasmflow::flow::FlowContext::new(
        "./target/wasm32-wasi/release/wasm_record_processor.wasm",
    ));

    let client = wasmflow::sources::kafka::init_client()
        .await
        .with_context(|| "Failed to initialize Kafka client.")?;

    let topics = client
        .list_topics()
        .await
        .with_context(|| "Failed to list topics")?;

    if !topics.is_empty() {
        let mut handles: Vec<JoinHandle<()>> = vec![];
        info!(topic = ?topics[0]);
        for partition in topics[0].partitions.iter() {
            let client_clone = client.clone();
            let flow_ctx_clone = fctx.clone();
            let partition_id = *partition;
            let topic_name = topics[0].name.clone();
            let handle = tokio::spawn(async move {
                let _res =
                    process_partition(client_clone, flow_ctx_clone, &topic_name, partition_id)
                        .await;
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
    fctx: Arc<FlowContext>,
    topic_name: &str,
    partition_id: i32,
) -> anyhow::Result<()> {
    let partition_client = client
        .partition_client(topic_name, partition_id)
        .await
        .with_context(|| format!("Error creating client for partition {partition_id}"))?;
    let mut offset = partition_client.get_offset(OffsetAt::Earliest).await?;
    loop {
        let result = partition_client
            .fetch_records(
                offset,       // offset
                1..1_000_000, // min..max bytes
                1_000,        // max wait time
            )
            .await
            .with_context(|| format!("Error fetching records from partition {partition_id}."))?;
        if !result.0.is_empty() {
            process_result(fctx.clone(), result.0, result.1, partition_id);
        }
        offset = result.1;
    }
}

fn process_result(
    fctx: Arc<FlowContext>,
    recs: Vec<RecordAndOffset>,
    _high_watermark: i64,
    _partition_id: i32,
) {
    let flow_state = FlowState::new();
    let mut store = Store::new(&fctx.engine, flow_state);
    let (wasm_rec_proc, _) = wasmflow::flow::RecordProcessor::instantiate(
        store.as_context_mut(),
        &fctx.module,
        &mut fctx.linker.clone(),
        |s| &mut s.data,
    )
    .expect("Could not create WASM instance.");

    let rec_headers: Vec<Vec<(&str, &[u8])>> = recs
        .iter()
        .map(|r| {
            r.record
                .headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_slice()))
                .collect()
        })
        .collect();

    let frecs: Vec<FlowRecord> = recs
        .iter()
        .zip(rec_headers.iter())
        .map(|(r, h)| FlowRecord {
            key: r.record.key.as_deref(),
            value: r.record.value.as_deref(),
            headers: h,
            offset: r.offset,
        })
        .collect();

    let resp = wasm_rec_proc
        .parse_records(store.as_context_mut(), &frecs)
        .expect("Error parsing record.");
    info!(wasm_output = ?resp);
}
