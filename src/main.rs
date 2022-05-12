use wasmflow::{flow::FlowProcessor, sinks::s3::S3Writer, sources::kafka::KafkaStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let conf = wasmflow::conf::read_config()?;
    let source = KafkaStream::new(&conf.sources[0]).await?;
    let s3_sink = S3Writer::new(&conf.sinks[0]).await?;
    let fctx = FlowProcessor::new(&conf.processors[0].module_path, source, s3_sink);
    fctx.start().await?;
    Ok(())
}
