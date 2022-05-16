use anyhow::Context;
use wasmflow::{
    flow::FlowProcessor, sinks::s3::BufferedS3Sink, sources::kafka::KafkaStreamBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    console_subscriber::init();
    let conf = wasmflow::conf::read_config()?;
    let source_stream_builder = KafkaStreamBuilder::new(&conf.sources[0]).await?;
    let s3_sink = BufferedS3Sink::new(&conf.sinks[0]).await?;
    let wasm_flow = FlowProcessor::new(
        &conf.processors[0].module_path,
        source_stream_builder,
        s3_sink,
    )
    .with_context(|| "Could not initialize WASM Flow")?;
    wasm_flow.run().await?;
    Ok(())
}
