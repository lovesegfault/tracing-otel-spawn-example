use std::env::VarError;
use std::fs::File;

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
    let tracer_provider = init_trace(&run_id);
    let tracer = tracer_provider.tracer("parent");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);
    let registry = Registry::default().with(stderr_layer).with(otel_layer);
    tracing::subscriber::set_global_default(registry)?;

    let args = Cli::parse();

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

    Ok(())
}

fn init_trace(run_id: &String) -> TracerProvider {
    let uuid = uuid::Uuid::now_v7();
    let writer =
        File::create(format!("{}/parent-{}.log", run_id, uuid)).expect("Failed to create log file");
    let exporter = SpanExporterBuilder::default().with_writer(writer).build();
    // let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();
    TracerProvider::builder()
        .with_simple_exporter(exporter)
        .build()
}
