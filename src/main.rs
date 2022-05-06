use wasmflow::{flow::FlowProcessor, sources::kafka::KafkaStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let fctx = FlowProcessor::new("./target/wasm32-wasi/release/wasm_record_processor.wasm");
    let stream = KafkaStream::new("ramr_test_topic", fctx).await?;
    stream.start().await?;
    Ok(())
}
