//! One bounded job, one VM. No timers, filesystem, network or dynamic module loader.
use anywhere_protocol::web_bridge::*;
use rquickjs::{Context, Function, Module, Promise, Runtime};
use serde_json::{json, Value};
use std::fmt;
use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub(super) type HostInvoke = Arc<dyn Fn(WebCall) -> WebResult<Value> + Send + Sync>;
pub(super) type Current = Arc<dyn Fn() -> bool + Send + Sync>;
pub(super) const MAX_OUTPUT: usize = 256 * 1024;

const MAX_HOST_CALLS: u64 = 1024;
const MAX_BUSINESS_CALLS: u64 = 128;
const MEMORY_LIMIT: usize = 32 * 1024 * 1024;
const STACK_LIMIT: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BackgroundFailure {
    MissingExport,
    ScriptException,
    Cancelled,
    CpuBudgetExceeded,
    MemoryLimit,
    OutputTooLarge,
    InvalidReturn,
}

impl fmt::Display for BackgroundFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingExport => "MISSING_EXPORT",
            Self::ScriptException => "SCRIPT_EXCEPTION",
            Self::Cancelled => "CANCELLED",
            Self::CpuBudgetExceeded => "CPU_BUDGET_EXCEEDED",
            Self::MemoryLimit => "MEMORY_LIMIT",
            Self::OutputTooLarge => "OUTPUT_TOO_LARGE",
            Self::InvalidReturn => "INVALID_RETURN",
        })
    }
}

impl std::error::Error for BackgroundFailure {}

fn classify(
    error: rquickjs::Error,
    ctx: &rquickjs::Ctx<'_>,
    interrupt: &AtomicU8,
) -> BackgroundFailure {
    match interrupt.load(Ordering::Relaxed) {
        1 => return BackgroundFailure::Cancelled,
        2 => return BackgroundFailure::CpuBudgetExceeded,
        _ => {}
    }
    if matches!(error, rquickjs::Error::Allocation) {
        return BackgroundFailure::MemoryLimit;
    }
    if matches!(error, rquickjs::Error::Exception) {
        // Compare only with the VM's resource error. The raw exception and stack
        // can contain input, paths or credentials and never leave this context.
        let exception = ctx.catch();
        if exception
            .as_exception()
            .and_then(|error| error.message())
            .as_deref()
            == Some("out of memory")
        {
            return BackgroundFailure::MemoryLimit;
        }
    }
    BackgroundFailure::ScriptException
}

/// Per-job accounting, separate from VM setup and JavaScript evaluation.
struct HostAdapter {
    calls: AtomicU64,
    business_calls: AtomicU64,
    paused: Arc<AtomicU64>,
    invoke: HostInvoke,
    current: Current,
}
impl HostAdapter {
    fn dispatch(&self, method: &str, params: &str) -> String {
        let result = self.call(method, params);
        match result {
            Ok(value) => json!({"value": value}),
            Err(error) => json!({"error": error}),
        }
        .to_string()
    }

    fn call(&self, method: &str, params: &str) -> WebResult<Value> {
        let sequence = self.calls.fetch_add(1, Ordering::Relaxed);
        if !(self.current)() {
            return Err(WebError::new(WebErrorCode::Cancelled));
        }
        if sequence >= MAX_HOST_CALLS {
            return Err(WebError::new(WebErrorCode::QuotaExceeded));
        }
        let frame = format!(
            "{{\"protocol\":{PROTOCOL},\"requestId\":\"bg_{sequence}\",\"method\":{},\"params\":{params}}}",
            serde_json::to_string(method).expect("method string")
        );
        let request = anywhere_plugin::web::wire::parse(&frame)?;
        // Text transport has its own total bound, leaving business slots available.
        let transport = matches!(
            request.call,
            WebCall::TextBegin { .. }
                | WebCall::TextChunk { .. }
                | WebCall::TextCommit { .. }
                | WebCall::TextRead { .. }
                | WebCall::TextCancel { .. }
        );
        if !transport && self.business_calls.fetch_add(1, Ordering::Relaxed) >= MAX_BUSINESS_CALLS {
            return Err(WebError::new(WebErrorCode::QuotaExceeded));
        }
        let started = Instant::now();
        let result = (self.invoke)(request.call);
        let elapsed = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.paused.fetch_add(elapsed, Ordering::Relaxed);
        result
    }
}

pub(super) fn run(
    source: &str,
    entrypoint: &str,
    input: &Value,
    invoke: HostInvoke,
    current: Current,
) -> Result<Value, BackgroundFailure> {
    run_with_budget(
        source,
        entrypoint,
        input,
        invoke,
        current,
        Duration::from_millis(250),
    )
}

pub(super) fn run_with_budget(
    source: &str,
    entrypoint: &str,
    input: &Value,
    invoke: HostInvoke,
    current: Current,
    budget: Duration,
) -> Result<Value, BackgroundFailure> {
    if !current() {
        return Err(BackgroundFailure::Cancelled);
    }
    let input = serde_json::to_string(input).map_err(|_| BackgroundFailure::ScriptException)?;
    if input.len() > MAX_TRANSFER_BYTES {
        return Err(BackgroundFailure::OutputTooLarge);
    }
    let runtime = Runtime::new().map_err(|_| BackgroundFailure::MemoryLimit)?;
    runtime.set_memory_limit(MEMORY_LIMIT);
    runtime.set_max_stack_size(STACK_LIMIT);
    let started = Instant::now();
    let paused = Arc::new(AtomicU64::new(0));
    let paused_interrupt = paused.clone();
    let active = current.clone();
    let interrupt = Arc::new(AtomicU8::new(0));
    let interrupted = interrupt.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        if !active() {
            interrupted.store(1, Ordering::Relaxed);
            return true;
        }
        if started.elapsed().saturating_sub(Duration::from_nanos(
            paused_interrupt.load(Ordering::Relaxed),
        )) > budget
        {
            interrupted.store(2, Ordering::Relaxed);
            return true;
        }
        false
    })));
    let context = Context::full(&runtime).map_err(|_| BackgroundFailure::MemoryLimit)?;
    let adapter = HostAdapter {
        calls: AtomicU64::new(0),
        business_calls: AtomicU64::new(0),
        paused,
        invoke,
        current: current.clone(),
    };
    let output = context.with(|ctx| {
        let classify = |error| classify(error, &ctx, &interrupt);
        let host = Function::new(ctx.clone(), move |method: String, params: String| {
            adapter.dispatch(&method, &params)
        })
        .map_err(classify)?;
        ctx.globals()
            .set("__anywhereInvoke", host)
            .map_err(classify)?;
        let (module, ready) = Module::declare(ctx.clone(), "plugin", source)
            .and_then(Module::eval)
            .map_err(classify)?;
        ready.finish::<()>().map_err(classify)?;
        let export = module
            .get::<_, rquickjs::Value<'_>>(entrypoint)
            .map_err(classify)?;
        let handler = export
            .into_function()
            .ok_or(BackgroundFailure::MissingExport)?;
        let call: Function = ctx
            .eval(
                "(handler, input) => Promise.resolve(handler(JSON.parse(input)))\
             .then(value => JSON.stringify(value ?? null))",
            )
            .map_err(classify)?;
        let promise: Promise = call.call((handler, input)).map_err(classify)?;
        let output = promise.finish::<rquickjs::Value<'_>>().map_err(classify)?;
        let output = output
            .into_string()
            .ok_or(BackgroundFailure::InvalidReturn)?;
        let output = output.to_cstring().map_err(classify)?;
        if output.len() > MAX_OUTPUT {
            return Err(BackgroundFailure::OutputTooLarge);
        }
        serde_json::from_str(output.as_str()).map_err(|_| BackgroundFailure::InvalidReturn)
    })?;
    if !current() {
        return Err(BackgroundFailure::Cancelled);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn no_host() -> HostInvoke {
        Arc::new(|_| Err(WebError::new(WebErrorCode::PermissionDenied)))
    }
    #[test]
    fn transport_budget_allows_full_text_but_business_calls_remain_bounded() {
        let invoke: HostInvoke = Arc::new(|_| Ok(Value::Null));
        let code = r#"export function parse() {
            for(let i=0;i<258;i++) {
                const r=JSON.parse(__anywhereInvoke('text.chunk',JSON.stringify({ticket:'a'.repeat(64),sequence:i,text:'x'})));
                if(r.error) throw Error(r.error.code);
            }
            let last;
            for(let i=0;i<129;i++) last=JSON.parse(__anywhereInvoke('context.get','{}'));
            return last.error.code;
        }"#;
        assert_eq!(
            run(code, "parse", &Value::Null, invoke, Arc::new(|| true)).unwrap(),
            json!("QUOTA_EXCEEDED")
        );
    }

    #[test]
    fn asynchronous_exports_and_host_promises_complete() {
        let invoke: HostInvoke = Arc::new(|call| match call {
            WebCall::ContextGet {} => Ok(json!({"kind":"text","text":"hello"})),
            _ => Err(WebError::new(WebErrorCode::PermissionDenied)),
        });
        let result = run("export async function parse() { const result = JSON.parse(__anywhereInvoke('context.get','{}')); return [result.value.text]; }", "parse", &Value::Null, invoke, Arc::new(|| true)).unwrap();
        assert_eq!(result, json!(["hello"]));
    }
    #[test]
    fn output_budget_counts_utf8_json_bytes_before_deserialization() {
        let evaluate = |expression: &str| {
            run(
                &format!("export async function parse() {{ return {expression}; }}"),
                "parse",
                &Value::Null,
                no_host(),
                Arc::new(|| true),
            )
        };
        // Quotes are part of the serialized byte limit too.
        let exact = evaluate(&format!("'x'.repeat({})", MAX_OUTPUT - 2)).unwrap();
        assert_eq!(exact.as_str().unwrap().len(), MAX_OUTPUT - 2);
        assert!(evaluate(&format!("'x'.repeat({})", MAX_OUTPUT - 1)).is_err());
        let unicode = evaluate(&format!("'中'.repeat({})", (MAX_OUTPUT - 2) / 3)).unwrap();
        assert_eq!(
            unicode.as_str().unwrap().chars().count(),
            (MAX_OUTPUT - 2) / 3
        );
        assert!(evaluate(&format!("'中'.repeat({})", (MAX_OUTPUT - 2) / 3 + 1)).is_err());
        assert!(evaluate("'x'.repeat(8 * 1024 * 1024)").is_err());
        assert_eq!(evaluate("null").unwrap(), Value::Null);
    }
    #[test]
    fn loops_allocations_unknown_exports_and_missing_runtime_apis_are_bounded() {
        for code in [
            "export function parse(){while(true){}}",
            "export function parse(){return new Array(50_000_000).fill('x')}",
            "export function wrong(){}",
            "export function parse(){return fetch('https://example.com')}",
        ] {
            assert!(run(code, "parse", &Value::Null, no_host(), Arc::new(|| true)).is_err());
        }
        assert!(run(
            "export function parse(){return 1}",
            "parse",
            &Value::Null,
            no_host(),
            Arc::new(|| false)
        )
        .is_err());
    }
}
