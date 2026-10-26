//! Third-party resolvers contribute namespaced data through the existing Context engine.
use super::{
    background_runtime::{self, Current, HostInvoke},
    catalog::WebPluginCatalog,
    projection,
};
use crate::application::session_model::LauncherSession;
use crate::core::context::{
    provider::{ContextProvider, ProviderDescriptor},
    registry::ContextProviderRegistry,
    ContextValue, ExecutionContext, ResolvedContext,
};
use anyhow::{ensure, Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::{web_bridge::*, web_plugin::*};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
struct BackgroundProvider {
    descriptor: ProviderDescriptor,
    entrypoint: String,
    file: crate::persistence::web_packages::WebResourceFile,
    permissions: Vec<WebPermission>,
    active: Arc<AtomicBool>,
    session: Arc<Mutex<LauncherSession>>,
    platform: Arc<dyn Platform>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Derived {
    #[serde(rename = "type")]
    type_id: String,
    value: Value,
}

impl WebPluginCatalog {
    pub(super) fn append_providers(
        &self,
        base: &ContextProviderRegistry,
        session: Arc<Mutex<LauncherSession>>,
        platform: Arc<dyn Platform>,
    ) -> Result<ContextProviderRegistry> {
        let mut next = base.clone();
        next.retain_contributors(|id| !id.starts_with("web:"));
        for registration in self.entries.values() {
            let Some(background) = &registration.manifest.background else {
                continue;
            };
            let file = registration
                .background_file
                .as_ref()
                .context("background resource not mounted")?;
            for resolver in &background.resolvers {
                let provider = Arc::new(BackgroundProvider {
                    descriptor: ProviderDescriptor {
                        id: format!("web.{}.{}", registration.manifest.id, resolver.id),
                        accepts: resolver.accepts.clone(),
                        produces: resolver.produces.clone(),
                        priority: -100,
                    },
                    entrypoint: resolver.entrypoint.clone(),
                    file: file.clone(),
                    permissions: registration.manifest.permissions.clone(),
                    active: registration.active.clone(),
                    session: session.clone(),
                    platform: platform.clone(),
                });
                next.register_owned(
                    provider.clone(),
                    Some(&format!("web:{}", registration.manifest.id)),
                )?;
                if resolver
                    .accepts
                    .iter()
                    .any(|kind| kind == anywhere_protocol::TEXT)
                {
                    let recognizer = provider.clone();
                    next.register_input_recognizer(
                        crate::core::context::input_recognizer::InputRecognizer {
                            id: provider.descriptor.id.clone(),
                            priority: -100,
                            recognize: Arc::new(move |text| {
                                let (id, revision) = {
                                    let session = recognizer.session.lock().ok()?;
                                    (session.state.session_id, session.state.revision)
                                };
                                let input = crate::features::text::provider::text_context(text)?;
                                recognizer
                                    .resolve_job(
                                        &input,
                                        &ExecutionContext::detached(id, revision),
                                        std::time::Duration::from_millis(75),
                                    )
                                    .ok()?
                                    .into_iter()
                                    .next()
                            }),
                        },
                        Some(&format!("web:{}", registration.manifest.id)),
                    )?;
                }
            }
        }
        Ok(next)
    }
}
#[async_trait::async_trait]
impl ContextProvider for BackgroundProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let provider = self.clone();
        let input = input.clone();
        let execution = execution.clone();
        tokio::task::spawn_blocking(move || {
            provider.resolve_job(&input, &execution, std::time::Duration::from_millis(250))
        })
        .await?
    }
}
impl BackgroundProvider {
    fn resolve_job(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
        budget: std::time::Duration,
    ) -> Result<Vec<ResolvedContext>> {
        let input = input.clone();
        let projected = projection::build_with_resources(&input, &self.permissions)?.0;
        let permissions = self.permissions.clone();
        let source_app = self
            .session
            .lock()
            .ok()
            .filter(|s| s.state.session_id == execution.session_id)
            .and_then(|s| {
                s.identity.frontmost.as_ref().map(|app| WebCurrentApp {
                    id: app.bundle_id.clone(),
                    name: app.name.clone(),
                })
            });
        let query = self
            .session
            .lock()
            .ok()
            .filter(|s| s.state.session_id == execution.session_id)
            .map(|s| s.state.input.clone())
            .unwrap_or_default();
        let platform = self.platform.clone();
        let snapshot = PluginSnapshot {
            version: u64::from(execution.revision),
            context: projected,
            input: permissions
                .contains(&WebPermission::InputRead {})
                .then_some(query),
        };
        let invoke: HostInvoke = Arc::new(move |call| {
            let denied = || WebError::new(WebErrorCode::PermissionDenied);
            Ok(match call {
                WebCall::StateGet {} | WebCall::StateSubscribe {} => json!(snapshot),
                WebCall::ContextGet {} => json!(snapshot.context),
                WebCall::InputGet {} if snapshot.input.is_some() => json!(snapshot.input),
                WebCall::CurrentApp {} | WebCall::SourceApp {}
                    if permissions.contains(&WebPermission::CurrentApp {}) =>
                {
                    json!(source_app)
                }
                WebCall::FrontmostApp {}
                    if permissions.contains(&WebPermission::FrontmostApp {}) =>
                {
                    json!(platform
                        .frontmost_application()
                        .map_err(|_| WebError::new(WebErrorCode::HostFailure))?
                        .map(|app| WebCurrentApp {
                            id: app.bundle_id,
                            name: app.name
                        }))
                }
                // Automatic Context resolution must never prompt, write or change the query.
                _ => return Err(denied()),
            })
        });
        let active = self.active.clone();
        let currency = execution.clone();
        let current: Current =
            Arc::new(move || active.load(Ordering::Acquire) && currency.is_current());
        let check = current.clone();
        let file = self.file.clone();
        let entrypoint = self.entrypoint.clone();
        let code = String::from_utf8(file.read()?)?;
        let output = background_runtime::run_with_budget(
            &code,
            &entrypoint,
            &Value::Null,
            invoke,
            current,
            budget,
        )?;
        ensure!(check(), "resolver expired");
        let output: Vec<Derived> = serde_json::from_value(output)?;
        ensure!(output.len() <= 16, "too many derived contexts");
        output
            .into_iter()
            .map(|derived| {
                ensure!(
                    self.descriptor.produces.contains(&derived.type_id),
                    "resolver returned undeclared context type"
                );
                Ok(ResolvedContext::new(
                    &derived.type_id,
                    ContextValue::Extension(serde_json::to_string(&derived.value)?.into()),
                    input.source,
                    &self.descriptor.id,
                ))
            })
            .collect()
    }
}
