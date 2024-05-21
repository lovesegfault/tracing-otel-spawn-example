use anyhow::Context;
use tokio::process::Command;
use tracing_subscriber::{filter::LevelFilter, layer::SubscriberExt, EnvFilter, Layer, Registry};

#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let run_id = std::env::var("RUN_ID").ok();
    if let Some(run_id) = run_id {}
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);
    let registry = Registry::default().with(stderr_layer);
    tracing::subscriber::set_global_default(registry)?;

    info!("starting child");

    info!("doing stuff!");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    info!("spawning grandchild");
    let status = Command::new("grandchild")
        .kill_on_drop(true)
        .status()
        .await
        .context("spawn grandchild")?;
    anyhow::ensure!(
        status.success(),
        "spawned grandchild failed with status: {status:?}"
    );

    info!("child done");
    Ok(())
}
