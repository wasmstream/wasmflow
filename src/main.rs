use wasmflow::{flow::FlowProcessor, sinks::s3::S3Writer, sources::kafka::KafkaStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let s3_sink = S3Writer::new("wasmflow-sink", "us-east-1").await?;
    let fctx = FlowProcessor::new("./target/wasm32-wasi/release/wasm_s3_sink.wasm", s3_sink);
    let stream = KafkaStream::new("ramr_test_topic", fctx).await?;
    stream.start().await?;
    Ok(())
}
