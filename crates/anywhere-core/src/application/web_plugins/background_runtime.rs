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
    runtime.set_memory_limit(32 * 1024 * 1024);
    runtime.set_max_stack_size(256 * 1024);
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
    let count = Arc::new(AtomicU64::new(0));
    let output = context.with(|ctx| -> rquickjs::Result<String> {
        let active = current.clone();
        ctx.globals().set("__anywhereInvoke", Function::new(ctx.clone(), move |method: String, params: String| -> String {
            let sequence = count.fetch_add(1, Ordering::Relaxed);
            let result = if !active() { Err(WebError::new(WebErrorCode::Cancelled)) }
                else if sequence >= 128 { Err(WebError::new(WebErrorCode::QuotaExceeded)) }
                else {
                    let frame = format!("{{\"protocol\":1,\"requestId\":\"bg_{sequence}\",\"method\":{},\"params\":{params}}}", serde_json::to_string(&method).expect("method string"));
                    anywhere_plugin::web::wire::parse(&frame).and_then(|request| {
                        let at = Instant::now();
                        let result = invoke(request.call);
                        paused.fetch_add(at.elapsed().as_nanos().min(u64::MAX as u128) as u64, Ordering::Relaxed);
                        result
                    })
                };
            match result { Ok(value) => json!({"value":value}), Err(error) => json!({"error":error}) }.to_string()
        })?)?;
        let (module, ready) = Module::declare(ctx.clone(), "plugin", source)?.eval()?;
        ready.finish::<()>()?;
        let handler: Function = module.get(entrypoint)?;
        let call: Function = ctx.eval("(handler, input) => Promise.resolve(handler(JSON.parse(input))).then(value => JSON.stringify(value ?? null))")?;
        let promise: Promise = call.call((handler, input))?;
        promise.finish::<String>()
    }).map_err(|_| anyhow::anyhow!("background script failed or exceeded its budget"))?;
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
