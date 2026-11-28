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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BackgroundError {
    pub(super) failure: BackgroundFailure,
    pub(super) stack: Option<String>,
}

impl BackgroundError {
    pub(super) fn new(failure: BackgroundFailure) -> Self {
        Self {
            failure,
            stack: None,
        }
    }
}

impl From<BackgroundFailure> for BackgroundError {
    fn from(failure: BackgroundFailure) -> Self {
        Self::new(failure)
    }
}

impl fmt::Display for BackgroundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.failure, formatter)
    }
}

impl std::error::Error for BackgroundError {}

fn dev_stack_enabled() -> bool {
    cfg!(debug_assertions) || std::env::var_os("ANYWHERE_DATA_DIR").is_some()
}

fn sanitize_stack(raw: &str) -> Option<String> {
    let mut out = String::new();
    for line in raw
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("at "))
        .take(8)
    {
        let cleaned: String = line.chars().filter(|ch| !ch.is_control()).collect();
        if cleaned.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(&cleaned);
        if out.len() >= 1024 {
            break;
        }
    }
    if out.is_empty() {
        return None;
    }
    if out.len() > 1024 {
        let mut end = 1024;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    Some(out)
}

enum RunOutcome {
    Ok(Value),
    Err(BackgroundError),
    InspectException,
}

fn classify_error(error: rquickjs::Error, interrupt: &AtomicU8) -> RunOutcome {
    match interrupt.load(Ordering::Relaxed) {
        1 => return RunOutcome::Err(BackgroundFailure::Cancelled.into()),
        2 => return RunOutcome::Err(BackgroundFailure::CpuBudgetExceeded.into()),
        _ => {}
    }
    if matches!(error, rquickjs::Error::Allocation) {
        return RunOutcome::Err(BackgroundFailure::MemoryLimit.into());
    }
    if matches!(error, rquickjs::Error::Exception) {
        return RunOutcome::InspectException;
    }
    RunOutcome::Err(BackgroundFailure::ScriptException.into())
}

fn inspect_exception(ctx: &rquickjs::Ctx<'_>) -> BackgroundError {
    let exception = ctx.catch();
    // QuickJS throws JS_NULL when heap exhaustion prevents allocating InternalError.
    if exception.is_null() {
        return BackgroundFailure::MemoryLimit.into();
    }
    if let Some(exc) = exception.as_exception() {
        if exc.message().as_deref() == Some("out of memory") {
            return BackgroundFailure::MemoryLimit.into();
        }
        // Never inspect or forward exc.message() (which may contain user input).
        // In debug or isolated dev mode, attach only sanitized call-frame locations.
        let stack = dev_stack_enabled()
            .then(|| exc.stack().and_then(|raw| sanitize_stack(&raw)))
            .flatten();
        return BackgroundError {
            failure: BackgroundFailure::ScriptException,
            stack,
        };
    }
    BackgroundFailure::ScriptException.into()
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
) -> Result<Value, BackgroundError> {
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
) -> Result<Value, BackgroundError> {
    if !current() {
        return Err(BackgroundFailure::Cancelled.into());
    }
    let input = serde_json::to_string(input)
        .map_err(|_| BackgroundError::new(BackgroundFailure::ScriptException))?;
    if input.len() > MAX_TRANSFER_BYTES {
        return Err(BackgroundFailure::OutputTooLarge.into());
    }
    let runtime =
        Runtime::new().map_err(|_| BackgroundError::new(BackgroundFailure::MemoryLimit))?;
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
    let context = Context::full(&runtime)
        .map_err(|_| BackgroundError::new(BackgroundFailure::MemoryLimit))?;
    let adapter = HostAdapter {
        calls: AtomicU64::new(0),
        business_calls: AtomicU64::new(0),
        paused,
        invoke,
        current: current.clone(),
    };
    let outcome = context.with(|ctx| -> RunOutcome {
        let classify = |error| classify_error(error, &interrupt);
        let host = match Function::new(ctx.clone(), move |method: String, params: String| {
            adapter.dispatch(&method, &params)
        }) {
            Ok(host) => host,
            Err(error) => return classify(error),
        };
        if let Err(error) = ctx.globals().set("__anywhereInvoke", host) {
            return classify(error);
        }
        let (module, ready) =
            match Module::declare(ctx.clone(), "plugin", source).and_then(Module::eval) {
                Ok(pair) => pair,
                Err(error) => return classify(error),
            };
        if let Err(error) = ready.finish::<()>() {
            return classify(error);
        }
        let export = match module.get::<_, rquickjs::Value<'_>>(entrypoint) {
            Ok(value) => value,
            Err(error) => return classify(error),
        };
        let Some(handler) = export.into_function() else {
            return RunOutcome::Err(BackgroundFailure::MissingExport.into());
        };
        let call: Function = match ctx.eval(
            "(handler, input) => Promise.resolve(handler(JSON.parse(input)))\
             .then(value => JSON.stringify(value ?? null))",
        ) {
            Ok(call) => call,
            Err(error) => return classify(error),
        };
        let promise: Promise = match call.call((handler, input)) {
            Ok(promise) => promise,
            Err(error) => return classify(error),
        };
        let output = match promise.finish::<rquickjs::Value<'_>>() {
            Ok(value) => value,
            Err(error) => return classify(error),
        };
        let Some(output) = output.into_string() else {
            return RunOutcome::Err(BackgroundFailure::InvalidReturn.into());
        };
        let output = match output.to_cstring() {
            Ok(text) => text,
            Err(error) => return classify(error),
        };
        if output.len() > MAX_OUTPUT {
            return RunOutcome::Err(BackgroundFailure::OutputTooLarge.into());
        }
        match serde_json::from_str(output.as_str()) {
            Ok(value) => RunOutcome::Ok(value),
            Err(_) => RunOutcome::Err(BackgroundFailure::InvalidReturn.into()),
        }
    });
    let output = match outcome {
        RunOutcome::Ok(value) => value,
        RunOutcome::Err(error) => return Err(error),
        RunOutcome::InspectException => {
            // Lift the memory limit outside `context.with` so QuickJS can read
            // the exception message/stack on OOM boundaries without re-borrowing Runtime.
            runtime.set_memory_limit(usize::MAX);
            return Err(context.with(|ctx| inspect_exception(&ctx)));
        }
    };
    if !current() {
        return Err(BackgroundFailure::Cancelled.into());
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
        for (code, expected) in [
            (
                "export function parse(){while(true){}}",
                BackgroundFailure::CpuBudgetExceeded,
            ),
            (
                "export function parse(){return new Array(50_000_000).fill('x')}",
                BackgroundFailure::MemoryLimit,
            ),
            (
                "export function parse(){const a=[];for(let i=0;i<2000;i++)a.push('x'.repeat(32768)+i);return a.length}",
                BackgroundFailure::MemoryLimit,
            ),
            ("export function wrong(){}", BackgroundFailure::MissingExport),
            (
                "export function parse(){return fetch('https://example.com')}",
                BackgroundFailure::ScriptException,
            ),
        ] {
            let err = run(code, "parse", &Value::Null, no_host(), Arc::new(|| true)).unwrap_err();
            assert_eq!(err.failure, expected, "unexpected failure for {code}");
        }
        let err = run(
            "export function parse(){return 1}",
            "parse",
            &Value::Null,
            no_host(),
            Arc::new(|| false),
        )
        .unwrap_err();
        assert_eq!(err.failure, BackgroundFailure::Cancelled);
    }
}
