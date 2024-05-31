use std::{
    cell::RefCell,
    fmt::Display,
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

    let run_id = run_id.unwrap_or_default();
    let _span = tracing::info_span!("grandchild", run_id = %run_id, self_id = %self_id);

    if let Ok(Some(parent_context)) = get_env_context_for_parent() {
        let pctx =
            opentelemetry::Context::map_current(|cx| cx.with_remote_span_context(parent_context));

        _span.set_parent(pctx);
    }
    set_env_context_for_child(_span.context().span().span_context());
    let _span_guard = _span.entered();

    info!("starting grandchild");

    info!("doing stuff!");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    info!("grandchild done");

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}

fn init_trace(run_id: &String, self_id: &String) -> anyhow::Result<TracerProvider> {
    let trace_logs_dir = format!("./logs/{run_id}");
    create_dir_all(&trace_logs_dir).context("create log dir for trace")?;

    let writer = File::create(format!("{}/grandchild-{}.json", &trace_logs_dir, self_id))
        .context("create log file")?;
    let exporter = SpanExporterBuilder::default()
        .with_writer(Arc::new(writer))
        .build();
    let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();
    Ok(TracerProvider::builder()
        .with_span_processor(processor)
        .build())
}
struct TraceParent {
    // https://www.w3.org/TR/trace-context-1/#traceparent-header
    version: String,
    trace_id: TraceId,
    parent_id: SpanId,
    trace_flags: TraceFlags,
}

impl From<TraceParent> for SpanContext {
    fn from(value: TraceParent) -> Self {
        SpanContext::new(
            value.trace_id,
            value.parent_id,
            value.trace_flags,
            true,
            TraceState::default(),
        )
    }
}

impl From<SpanContext> for TraceParent {
    fn from(span_ctx: SpanContext) -> Self {
        Self {
            version: "00".to_string(), // WARNING: This is hardcoded in the current spec but may change
            trace_id: span_ctx.trace_id(),
            parent_id: span_ctx.span_id(),
            trace_flags: span_ctx.trace_flags(),
        }
    }
}

impl From<&SpanContext> for TraceParent {
    fn from(span_ctx: &SpanContext) -> Self {
        Self {
            version: "00".to_string(), // WARNING: This is hardcoded in the current spec but may change
            trace_id: span_ctx.trace_id(),
            parent_id: span_ctx.span_id(),
            trace_flags: span_ctx.trace_flags(),
        }
    }
}

impl TryFrom<&String> for TraceParent {
    type Error = anyhow::Error;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        if let [version, trace_id, parent_id, trace_flags] =
            value.split('-').collect::<Vec<_>>()[..]
        {
            // TODO: TRACESTATE https://www.w3.org/TR/trace-context-1/#tracestate-header
            let trace_parent = TraceParent {
                version: version.to_string(),
                trace_id: TraceId::from_hex(trace_id).unwrap(),
                parent_id: SpanId::from_hex(parent_id).unwrap(),
                trace_flags: TraceFlags::new(trace_flags.parse::<u8>().unwrap()),
            };
            Ok(trace_parent)
        } else {
            anyhow::bail!("Invalid TRACEPARENT format");
        }
    }
}

impl Display for TraceParent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{version}-{trace_id}-{parent_id}-{trace_flags}",
            version = self.version,
            trace_id = self.trace_id,
            parent_id = self.parent_id,
            trace_flags = self.trace_flags.to_u8()
        )
    }
}

fn set_env_context_for_child(span_ctx: &SpanContext) {
    let trace_parent = TraceParent::from(span_ctx);
    std::env::set_var("TRACEPARENT", trace_parent.to_string());
    // TODO: TRACESTATE https://www.w3.org/TR/trace-context-1/#tracestate-header
}

fn get_env_context_for_parent() -> anyhow::Result<Option<SpanContext>> {
    if let Ok(trace_parent_env_var) = std::env::var("TRACEPARENT") {
        if let Ok(trace_parent) = TraceParent::try_from(&trace_parent_env_var) {
            info!("TRACEPARENT: {trace_parent}");
            Ok(Some(trace_parent.into()))
        } else {
            error!("Invalid TRACEPARENT: {trace_parent_env_var}");
            anyhow::bail!("Invalid TRACEPARENT format");
        }
    } else {
        info!("No TRACEPARENT found");
        Ok(None)
    }
}
