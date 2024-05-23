use std::{
    cell::RefCell,
    fs::{create_dir_all, File},
    rc::Rc,
    sync::Arc,
};

use anyhow::Context;
use opentelemetry::{
    global::ObjectSafeTracerProvider,
    trace::{Span, Tracer, TracerProvider as _},
};
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _run_id = std::env::var("RUN_ID").ok();
    let run_id = _run_id.clone().unwrap_or("NULL".to_string());
    let self_id = uuid::Uuid::now_v7().to_string();

    // HACK:Use RC to keep TracerProvider from being dropped
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1625
    let rc_tracer_provider: Rc<RefCell<Option<TracerProvider>>> = Rc::new(RefCell::new(None));
    let otel_layer = if let Some(run_id) = &_run_id {
        let mut ref_tracer_provider = rc_tracer_provider.borrow_mut();
        let tracer_provider =
            init_trace(run_id, &self_id).expect("Failed to set up trace provider");
        *ref_tracer_provider = Some(tracer_provider);
        let tracer_provider = ref_tracer_provider.as_ref().unwrap();
        let tracer = tracer_provider.tracer("grandchild-tracer");

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        Some(otel_layer)
    } else {
        None
    };
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env_filter);
    Registry::default()
        .with(stderr_layer)
        .with(otel_layer)
        .init();

    let _span_guard =
        tracing::info_span!("child", run_id = %&run_id, self_id = %&self_id).entered();

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

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn init_trace(run_id: &String, self_id: &String) -> anyhow::Result<TracerProvider> {
    let trace_logs_dir = format!("./logs/{run_id}");
    create_dir_all(&trace_logs_dir).context("create log dir for trace")?;

    let writer = File::create(format!("{}/child-{}.json", &trace_logs_dir, self_id))
        .context("create log file")?;
    let exporter = SpanExporterBuilder::default()
        .with_writer(Arc::new(writer))
        .build();
    let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();
    Ok(TracerProvider::builder()
        .with_span_processor(processor)
        .build())
}
