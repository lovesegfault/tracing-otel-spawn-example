use std::fs::File;
use std::sync::Arc;
use std::{env::VarError, fs::create_dir_all};

use anyhow::Context;
use clap::Parser;
use opentelemetry::trace::{Span, Tracer, TracerProvider as _};
use opentelemetry_sdk::{
    runtime,
    trace::{BatchSpanProcessor, TracerProvider},
};
use opentelemetry_stdout::SpanExporterBuilder;
use tokio::process::Command;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter,
    Layer, Registry,
};

#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn};

#[derive(Debug, Parser)]
struct Cli {
    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    SpawnSelf,
    SpawnChild,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let run_id = std::env::var("RUN_ID").or_else(|err| match err {
        VarError::NotUnicode(_) => anyhow::bail!("RUN_ID is not unicode"),
        VarError::NotPresent => {
            let uuid = uuid::Uuid::now_v7().to_string();
            std::env::set_var("RUN_ID", &uuid);
            Ok(uuid)
        }
    })?;
    let self_id = uuid::Uuid::now_v7().to_string();
    let tracer_provider = init_trace(&run_id, &self_id).expect("Failed");
    let tracer = tracer_provider.tracer("parent-tracer");

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let stderr_env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_env_filter);
    // Sets registry as global default subscriber
    Registry::default()
        .with(stderr_layer)
        .with(otel_layer)
        .init();

    let args = Cli::parse();
    let _span_guard = tracing::info_span!("parent", run_id = %run_id, self_id = %self_id).entered();

    info!(%run_id, "starting parent");
    let status = match args.command {
        SubCommand::SpawnSelf => {
            info!("re-spawning parent");
            Command::new("parent")
                .arg("spawn-child")
                .kill_on_drop(true)
                .status()
                .await
                .context("spawn parent")?
        }
        SubCommand::SpawnChild => {
            info!("spawning child");
            Command::new("child")
                .kill_on_drop(true)
                .status()
                .await
                .context("spawn child")?
        }
    };

    anyhow::ensure!(
        status.success(),
        "spawned process failed with status: {status:?}"
    );

    info!(command=?args.command, "parent done");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn init_trace(run_id: &String, self_id: &String) -> anyhow::Result<TracerProvider> {
    let trace_logs_dir = format!("./logs/{run_id}");
    create_dir_all(&trace_logs_dir).context("create log dir for trace")?;

    let writer = File::create(format!("{}/parent-{}.json", &trace_logs_dir, self_id))
        .context("create log file")?;
    let exporter = SpanExporterBuilder::default()
        .with_writer(Arc::new(writer))
        .build();
    let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();
    Ok(TracerProvider::builder()
        .with_span_processor(processor)
        .build())
}
