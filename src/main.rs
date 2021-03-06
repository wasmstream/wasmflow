use anyhow::Context;
use opentelemetry_otlp::WithExportConfig;
use wasmflow::{
    flow::FlowProcessor, sinks::s3::BufferedS3Sink, sources::kafka::create_kafka_consumer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _pipeline = init_meter().with_context(|| "Could not initialize metrics exporter")?;

    console_subscriber::init();

    let cfg = wasmflow::conf::read_config()?;
    let kafka_consumer = create_kafka_consumer(&cfg.sources[0])?;
    let s3_sink = BufferedS3Sink::new(&cfg.sinks[0]).await?;
    let meter = opentelemetry::global::meter("wasmflow");
    let wasm_flow = FlowProcessor::new(
        &cfg.processors[0].module_path,
        meter,
        kafka_consumer,
        s3_sink,
    )
    .with_context(|| "Could not initialize WASM Flow")?;
    wasm_flow.run().await?;
    Ok(())
}

fn init_meter() -> opentelemetry::metrics::Result<opentelemetry::sdk::metrics::PushController> {
    let export_config = opentelemetry_otlp::ExportConfig {
        endpoint: "http://localhost:4317".to_string(),
        timeout: std::time::Duration::from_secs(3),
        protocol: opentelemetry_otlp::Protocol::Grpc,
    };
    opentelemetry_otlp::new_pipeline()
        .metrics(tokio::spawn, opentelemetry::util::tokio_interval_stream)
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_export_config(export_config),
        )
        .with_period(std::time::Duration::from_secs(3))
        .with_aggregator_selector(opentelemetry::sdk::metrics::selectors::simple::Selector::Exact)
        .build()
}
