use wasmflow::{flow::FlowProcessor, sinks::s3::S3Writer, sources::kafka::KafkaStreamBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let conf = wasmflow::conf::read_config()?;
    let source_stream_builder = KafkaStreamBuilder::new(&conf.sources[0]).await?;
    let s3_sink = S3Writer::new(&conf.sinks[0]).await?;
    let wasm_flow = FlowProcessor::new(
        &conf.processors[0].module_path,
        source_stream_builder,
        s3_sink,
    );
    wasm_flow.run().await?;
    Ok(())
}
