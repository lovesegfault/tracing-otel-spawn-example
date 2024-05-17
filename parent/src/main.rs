use anyhow::Context;
use clap::Parser;
use tokio::process::Command;
use tracing_subscriber::{filter::LevelFilter, layer::SubscriberExt, EnvFilter, Layer, Registry};

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
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);
    let registry = Registry::default().with(stderr_layer);
    tracing::subscriber::set_global_default(registry)?;

    let args = Cli::parse();

    info!("starting parent");
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
        },
    };

    anyhow::ensure!(status.success(), "spawned process failed with status: {status:?}");

    info!(command=?args.command, "parent done");

    Ok(())
}
