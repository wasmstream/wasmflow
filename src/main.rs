use tracing::info;
use wasmflow::{flow::FlowProcessor, sinks::s3::S3Writer, sources::kafka::KafkaStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let conf = wasmflow::conf::read_config()?;
    info!(conf=?conf);
    let s3_sink = S3Writer::new(&conf.sinks[0]).await?;
    let fctx = FlowProcessor::new(&conf.processors[0].module_path, s3_sink);
    let stream = KafkaStream::new(&conf.sources[0], fctx).await?;
    stream.start().await?;
    Ok(())
}
