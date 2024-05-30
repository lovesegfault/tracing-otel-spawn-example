use std::{
    cell::RefCell,
    fs::{create_dir_all, File},
    rc::Rc,
    sync::Arc,
};

use anyhow::Context;
use opentelemetry::trace::{
    Span, SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, Tracer,
    TracerProvider as _,
};
use opentelemetry_sdk::{
    runtime,
    trace::{BatchSpanProcessor, TracerProvider},
};
use opentelemetry_stdout::SpanExporterBuilder;

use tokio::process::Command;

use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter,
    Layer, Registry,
};

#[allow(unused_imports)]
use tracing::{debug, error, info, span, trace, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let self_id = uuid::Uuid::now_v7().to_string();
    let run_id = std::env::var("RUN_ID").ok();

    // HACK:Use RC to keep TracerProvider from being dropped
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1625
    let rc_tracer_provider: Rc<RefCell<Option<TracerProvider>>> = Rc::new(RefCell::new(None));
    let otel_layer = if let Some(run_id) = &run_id {
        let mut ref_tracer_provider = rc_tracer_provider.borrow_mut();
        let tracer_provider =
            init_trace(run_id, &self_id).expect("Failed to set up trace provider");
        *ref_tracer_provider = Some(tracer_provider);
        let tracer_provider = ref_tracer_provider.as_ref().unwrap();
        let tracer = tracer_provider.tracer("child-tracer");

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

    let run_id = run_id.unwrap_or_default();
    let _span = tracing::info_span!("child", run_id = %run_id, self_id = %self_id);

    if let Ok(Some(parent_context)) = get_env_context_for_parent() {
        let pctx =
            opentelemetry::Context::map_current(|cx| cx.with_remote_span_context(parent_context));

        _span.set_parent(pctx);
    }
    set_env_context_for_child(_span.context().span().span_context());
    let _span_guard = _span.entered();

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

fn set_env_context_for_child(span_ctx: &SpanContext) {
    // https://www.w3.org/TR/trace-context-1/#traceparent-header
    let version = "00"; // WARNING: This is hardcoded in the current spec but may change
    let trace_id = span_ctx.trace_id();
    // Sets parent_id for the child
    let parent_id = span_ctx.span_id();
    let trace_flags = span_ctx.trace_flags().to_u8();
    let trace_parent = format!("{version}-{trace_id}-{parent_id}-{trace_flags}");
    std::env::set_var("TRACEPARENT", trace_parent);
    // TODO: TRACESTATE https://www.w3.org/TR/trace-context-1/#tracestate-header
}

fn get_env_context_for_parent() -> anyhow::Result<Option<SpanContext>> {
    if let Ok(trace_parent) = std::env::var("TRACEPARENT") {
        if let [version, trace_id, parent_id, trace_flags] =
            trace_parent.split('-').collect::<Vec<_>>()[..]
        {
            // TODO: TRACESTATE https://www.w3.org/TR/trace-context-1/#tracestate-header
            let parent_context = SpanContext::new(
                TraceId::from_hex(trace_id).unwrap(),
                SpanId::from_hex(parent_id).unwrap(),
                TraceFlags::new(trace_flags.parse::<u8>().unwrap()),
                true,
                TraceState::default(),
            );
            println!("WE GOT EM BOYS");
            Ok(Some(parent_context))
        } else {
            println!("Invalid TRACEPARENT format: {trace_parent}");
            anyhow::bail!("Invalid TRACEPARENT format");
        }
    } else {
        println!("No TRACEPARENT found");
        Ok(None)
    }
}
