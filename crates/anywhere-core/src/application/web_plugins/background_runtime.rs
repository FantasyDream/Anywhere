//! One bounded job, one VM. No timers, filesystem, network or dynamic module loader.
use anyhow::{ensure, Result};
use anywhere_protocol::web_bridge::*;
use rquickjs::{Context, Function, Module, Promise, Runtime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
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

/// Per-job accounting, separate from VM setup and JavaScript evaluation.
struct HostAdapter {
    calls: AtomicU64,
    business_calls: AtomicU64,
    paused: Arc<AtomicU64>,
    invoke: HostInvoke,
    current: Current,
}
impl HostAdapter {
    fn dispatch(&self, method: String, params: String) -> String {
        let result = self.call(&method, &params);
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
            "{{\"protocol\":1,\"requestId\":\"bg_{sequence}\",\"method\":{},\"params\":{params}}}",
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
) -> Result<Value> {
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
) -> Result<Value> {
    ensure!(current(), "background job expired");
    let input = serde_json::to_string(input)?;
    ensure!(
        input.len() <= MAX_TRANSFER_BYTES,
        "background input exceeds budget"
    );
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(MEMORY_LIMIT);
    runtime.set_max_stack_size(STACK_LIMIT);
    let started = Instant::now();
    let paused = Arc::new(AtomicU64::new(0));
    let paused_interrupt = paused.clone();
    let active = current.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        !active()
            || started.elapsed().saturating_sub(Duration::from_nanos(
                paused_interrupt.load(Ordering::Relaxed),
            )) > budget
    })));
    let context = Context::full(&runtime)?;
    let adapter = HostAdapter {
        calls: AtomicU64::new(0),
        business_calls: AtomicU64::new(0),
        paused,
        invoke,
        current: current.clone(),
    };
    let output = context
        .with(|ctx| -> rquickjs::Result<String> {
            let host = Function::new(ctx.clone(), move |method: String, params: String| {
                adapter.dispatch(method, params)
            })?;
            ctx.globals().set("__anywhereInvoke", host)?;
            let (module, ready) = Module::declare(ctx.clone(), "plugin", source)?.eval()?;
            ready.finish::<()>()?;
            let handler: Function = module.get(entrypoint)?;
            let call: Function = ctx.eval(
                "(handler, input) => Promise.resolve(handler(JSON.parse(input)))\
             .then(value => JSON.stringify(value ?? null))",
            )?;
            let promise: Promise = call.call((handler, input))?;
            promise.finish::<String>()
        })
        .map_err(|_| anyhow::anyhow!("background script failed or exceeded its budget"))?;
    ensure!(
        current() && output.len() <= MAX_OUTPUT,
        "background output expired or exceeds budget"
    );
    Ok(serde_json::from_str(&output)?)
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
