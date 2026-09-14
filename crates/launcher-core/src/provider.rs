use crate::context::{ContextValue, ResolvedContext};
use anyhow::{bail, Result};
use async_trait::async_trait;
use launcher_platform::{ApplicationInfo, Platform};
use launcher_protocol::*;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::broadcast;

pub const MAX_DEPTH: u8 = 3;
const MAX_CONTEXTS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionClass {
    Immediate,
    AsyncLocal,
    Network,
    Ai,
}

#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub accepts: Vec<ContextTypeId>,
    pub produces: Vec<ContextTypeId>,
    pub execution_class: ExecutionClass,
    pub priority: i32,
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub session_id: SessionId,
    pub revision: Revision,
    pub(crate) current: Arc<AtomicU64>,
    pub(crate) events: broadcast::Sender<LauncherEvent>,
}

pub(crate) fn stamp(session: SessionId, revision: Revision) -> u64 {
    ((session as u64) << 32) | revision as u64
}

impl ExecutionContext {
    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire) == stamp(self.session_id, self.revision)
    }
    pub(crate) fn emit(&self, event: LauncherEvent) {
        if self.is_current() {
            let _ = self.events.send(event);
        }
    }
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>>;
}

/// Activation root provider. Captures before any host is told to show a window.
pub struct FrontmostApplicationProvider;
impl FrontmostApplicationProvider {
    pub async fn capture(platform: Arc<dyn Platform>) -> Result<Vec<ResolvedContext>> {
        tracing::info!(
            provider_id = "platform.frontmost_application",
            "provider_started"
        );
        let application =
            tokio::task::spawn_blocking(move || platform.frontmost_application()).await??;
        tracing::info!(
            provider_id = "platform.frontmost_application",
            count = application.is_some() as u8,
            "provider_completed"
        );
        Ok(application
            .into_iter()
            .map(|app| {
                ResolvedContext::new(
                    APPLICATION,
                    ContextValue::Application {
                        bundle_id: app.bundle_id,
                        name: app.name,
                    },
                    ContextSource::ForegroundApplication,
                    "platform.frontmost_application",
                )
            })
            .collect())
    }
}

pub struct CurrentDirectoryProvider {
    descriptor: ProviderDescriptor,
    platform: Arc<dyn Platform>,
}
impl CurrentDirectoryProvider {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self {
            platform,
            descriptor: ProviderDescriptor {
                id: "platform.current_directory".into(),
                accepts: vec![APPLICATION.into()],
                produces: vec![DIRECTORY.into()],
                execution_class: ExecutionClass::AsyncLocal,
                priority: 0,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for CurrentDirectoryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        if !execution.is_current() || execution.revision != 0 {
            return Ok(vec![]);
        }
        let ContextValue::Application { bundle_id, name } = &input.value else {
            return Ok(vec![]);
        };
        let application = ApplicationInfo {
            bundle_id: bundle_id.clone(),
            name: name.clone(),
        };
        let platform = self.platform.clone();
        let directory =
            tokio::task::spawn_blocking(move || platform.application_directory(&application))
                .await??;
        Ok(directory
            .into_iter()
            .map(|directory| {
                ResolvedContext::new(
                    DIRECTORY,
                    ContextValue::Directory(directory),
                    ContextSource::ApplicationContext,
                    &self.descriptor.id,
                )
            })
            .collect())
    }
}

#[derive(Default)]
pub struct ContextGraph {
    providers: Vec<Arc<dyn ContextProvider>>,
}
impl ContextGraph {
    pub fn register(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        let descriptor = provider.descriptor();
        if self
            .providers
            .iter()
            .any(|p| p.descriptor().id == descriptor.id)
        {
            bail!("duplicate provider id: {}", descriptor.id);
        }
        if !matches!(
            descriptor.execution_class,
            ExecutionClass::Immediate | ExecutionClass::AsyncLocal
        ) {
            bail!("Network and AI providers are outside MVP scope");
        }
        self.providers.push(provider);
        self.providers.sort_by(|a, b| {
            b.descriptor()
                .priority
                .cmp(&a.descriptor().priority)
                .then_with(|| a.descriptor().id.cmp(&b.descriptor().id))
        });
        Ok(())
    }

    pub async fn enrich(
        &self,
        seeds: Vec<ResolvedContext>,
        execution: &ExecutionContext,
    ) -> (Vec<ResolvedContext>, Vec<String>) {
        let mut contexts = Vec::new();
        let mut equivalents = HashSet::new();
        let mut visited = HashSet::new();
        let mut diagnostics = Vec::new();
        for seed in seeds {
            insert(&mut contexts, &mut equivalents, seed, execution);
        }
        let mut cursor = 0;
        while cursor < contexts.len() && execution.is_current() {
            let input = contexts[cursor].clone();
            cursor += 1;
            if input.depth >= MAX_DEPTH {
                continue;
            }
            for provider in &self.providers {
                if !execution.is_current() {
                    break;
                }
                let descriptor = provider.descriptor();
                if !descriptor.accepts.contains(&input.type_id)
                    || !visited.insert((
                        descriptor.id.clone(),
                        input.id.clone(),
                        execution.session_id,
                        execution.revision,
                    ))
                {
                    continue;
                }
                tracing::info!(
                    session_id = execution.session_id,
                    revision = execution.revision,
                    provider_id = descriptor.id,
                    source_context_id = input.id,
                    "provider_started"
                );
                // AsyncLocal implementations must move blocking OS work off the runtime worker.
                let result = provider.resolve(&input, execution).await;
                if !execution.is_current() {
                    tracing::info!(
                        session_id = execution.session_id,
                        revision = execution.revision,
                        "stale_provider_discarded"
                    );
                    break;
                }
                match result {
                    Ok(outputs) => {
                        for mut output in outputs {
                            if !descriptor.produces.contains(&output.type_id)
                                || !output.confidence.is_finite()
                                || !(0.0..=1.0).contains(&output.confidence)
                            {
                                diagnostics
                                    .push(format!("{} returned an invalid context", descriptor.id));
                                continue;
                            }
                            if contexts.len() >= MAX_CONTEXTS {
                                diagnostics
                                    .push(format!("context graph reached {MAX_CONTEXTS} contexts"));
                                break;
                            }
                            output.provider_id = descriptor.id.clone();
                            output.depth = input.depth + 1;
                            output.source_context_id = Some(input.id.clone());
                            if input.source == ContextSource::UserInput {
                                output.source = ContextSource::UserInput;
                            }
                            insert(&mut contexts, &mut equivalents, output, execution);
                        }
                        execution.emit(LauncherEvent::ProviderCompleted {
                            session_id: execution.session_id,
                            revision: execution.revision,
                            provider_id: descriptor.id.clone(),
                        });
                        tracing::info!(
                            session_id = execution.session_id,
                            revision = execution.revision,
                            provider_id = descriptor.id,
                            "provider_completed"
                        );
                    }
                    Err(error) => {
                        let message = format!("{}: {error:#}", descriptor.id);
                        tracing::error!(session_id = execution.session_id, revision = execution.revision, provider_id = descriptor.id, error = %error, "provider_failed");
                        execution.emit(LauncherEvent::ProviderFailed {
                            session_id: execution.session_id,
                            revision: execution.revision,
                            provider_id: descriptor.id.clone(),
                            message: message.clone(),
                        });
                        diagnostics.push(message);
                    }
                }
            }
        }
        (contexts, diagnostics)
    }
}

fn insert(
    contexts: &mut Vec<ResolvedContext>,
    equivalents: &mut HashSet<(String, String, ContextSource)>,
    mut context: ResolvedContext,
    execution: &ExecutionContext,
) {
    // Source is significant: explicit user input must not be swallowed by ambient context.
    if !execution.is_current()
        || !equivalents.insert((
            context.type_id.clone(),
            context.value.text(),
            context.source,
        ))
    {
        return;
    }
    context.session_id = execution.session_id;
    context.revision = execution.revision;
    context.id = format!("c{}", contexts.len());
    execution.emit(LauncherEvent::ContextAdded {
        context: context.descriptor(),
    });
    contexts.push(context);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Chain {
        descriptor: ProviderDescriptor,
        calls: Arc<AtomicUsize>,
        duplicate: bool,
    }
    #[async_trait]
    impl ContextProvider for Chain {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            input: &ResolvedContext,
            _: &ExecutionContext,
        ) -> Result<Vec<ResolvedContext>> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let value = if self.duplicate {
                input.value.text()
            } else {
                format!("{}+", input.value.text())
            };
            Ok(vec![ResolvedContext::new(
                TEXT,
                ContextValue::Text(value),
                input.source,
                "chain",
            )])
        }
    }
    pub(crate) fn execution() -> ExecutionContext {
        ExecutionContext {
            session_id: 1,
            revision: 0,
            current: Arc::new(AtomicU64::new(stamp(1, 0))),
            events: broadcast::channel(32).0,
        }
    }
    #[tokio::test]
    async fn enrichment_caps_depth_deduplicates_and_tracks_provenance() {
        for duplicate in [true, false] {
            let calls = Arc::new(AtomicUsize::new(0));
            let mut graph = ContextGraph::default();
            graph
                .register(Arc::new(Chain {
                    descriptor: ProviderDescriptor {
                        id: "chain".into(),
                        accepts: vec![TEXT.into()],
                        produces: vec![TEXT.into()],
                        execution_class: ExecutionClass::Immediate,
                        priority: 0,
                    },
                    calls: calls.clone(),
                    duplicate,
                }))
                .unwrap();
            let seed = ResolvedContext::new(
                TEXT,
                ContextValue::Text("start".into()),
                ContextSource::UserInput,
                "test",
            );
            let (contexts, diagnostics) =
                graph.enrich(vec![seed.clone(), seed], &execution()).await;
            assert!(diagnostics.is_empty());
            assert_eq!(contexts.len(), if duplicate { 1 } else { 4 });
            assert_eq!(calls.load(Ordering::Relaxed), if duplicate { 1 } else { 3 });
            assert!(contexts
                .iter()
                .all(|c| c.depth <= 3 && c.session_id == 1 && c.revision == 0));
            if !duplicate {
                assert_eq!(contexts[1].source_context_id.as_deref(), Some("c0"));
            }
        }
    }
}
