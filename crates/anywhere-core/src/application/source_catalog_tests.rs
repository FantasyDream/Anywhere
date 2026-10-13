use super::{Application, LauncherApplication};
use crate::core::{
    action::descriptor::ActionRegistry,
    context::{registry::ContextProviderRegistry, ExecutionContext},
    ranking::usage::MemoryUsage,
    search::candidate_source::*,
};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, Platform};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

struct QuietPlatform;
impl Platform for QuietPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn open_url(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn open_file(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn reveal_file(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn open_directory(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn open_terminal(&self, _: &Path) -> Result<()> {
        Ok(())
    }
    fn copy_text(&self, _: &str) -> Result<()> {
        Ok(())
    }
}
struct EmptySource(&'static str);
impl CandidateSource for EmptySource {
    fn id(&self) -> &'static str {
        self.0
    }
    fn candidates(&self, _: &SearchContext<'_>) -> Vec<crate::core::search::candidate::Candidate> {
        vec![]
    }
}
struct PendingSource {
    started: tokio::sync::Notify,
    ticket: Mutex<Option<ExecutionContext>>,
    resets: AtomicUsize,
}
#[async_trait::async_trait]
impl AsyncCandidateSource for PendingSource {
    fn id(&self) -> &'static str {
        "addon.async"
    }
    fn reset(&self) {
        self.resets.fetch_add(1, Ordering::SeqCst);
    }
    fn debounce(&self) -> Duration {
        Duration::ZERO
    }
    async fn candidates(
        &self,
        _: &str,
        ticket: ExecutionContext,
    ) -> Result<Vec<crate::core::search::candidate::Candidate>> {
        *self.ticket.lock().unwrap() = Some(ticket.clone());
        self.started.notify_one();
        ticket.superseded().await;
        Ok(vec![])
    }
}

#[tokio::test]
async fn synchronous_and_async_sources_share_atomic_install_and_cancel_on_removal() {
    let mut app = Application::new(
        Arc::new(QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.register_candidate_source(Arc::new(EmptySource("base")));
    let slow = Arc::new(PendingSource {
        started: Default::default(),
        ticket: Default::default(),
        resets: AtomicUsize::new(0),
    });
    app.replace_extension_runtime(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet {
            synchronous: vec![Arc::new(EmptySource("addon.sync"))],
            asynchronous: vec![slow.clone()],
        },
        || Ok(()),
    )
    .unwrap();
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_context_input(
        id,
        "hello".into(),
        1,
        None,
        anywhere_protocol::InputMode::Search,
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), slow.started.notified())
        .await
        .unwrap();
    assert!(slow.ticket.lock().unwrap().as_ref().unwrap().is_current());
    // Failed publication preserves both source kinds and does not cancel the old catalog.
    assert!(app
        .replace_extension_runtime(
            ActionRegistry::default(),
            ContextProviderRegistry::default(),
            SourceSet::default(),
            || anyhow::bail!("disk full")
        )
        .is_err());
    assert!(slow.ticket.lock().unwrap().as_ref().unwrap().is_current());
    assert_eq!(slow.resets.load(Ordering::SeqCst), 0);
    app.replace_extension_runtime(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        || Ok(()),
    )
    .unwrap();
    assert!(!slow.ticket.lock().unwrap().as_ref().unwrap().is_current());
    assert_eq!(slow.resets.load(Ordering::SeqCst), 1);
    let catalog = app.resolver.sources.lock().unwrap();
    assert_eq!(
        catalog
            .synchronous()
            .iter()
            .map(|s| s.id())
            .collect::<Vec<_>>(),
        ["base"]
    );
    assert!(catalog.asynchronous().is_empty());
}
