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

pub(super) struct QuietPlatform;
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
    app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet {
            synchronous: vec![Arc::new(EmptySource("addon.sync"))],
            asynchronous: vec![slow.clone()],
            ..Default::default()
        },
        app.resolver.catalog.policy(),
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
        .publish_feature_catalog(
            ActionRegistry::default(),
            ContextProviderRegistry::default(),
            SourceSet::default(),
            app.resolver.catalog.policy(),
            || anyhow::bail!("disk full")
        )
        .is_err());
    assert!(slow.ticket.lock().unwrap().as_ref().unwrap().is_current());
    assert_eq!(slow.resets.load(Ordering::SeqCst), 0);
    app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        app.resolver.catalog.policy(),
        || Ok(()),
    )
    .unwrap();
    assert!(!slow.ticket.lock().unwrap().as_ref().unwrap().is_current());
    assert_eq!(slow.resets.load(Ordering::SeqCst), 1);
    let sources = app.resolver.catalog.synchronous_sources().unwrap();
    assert_eq!(sources.iter().map(|s| s.id()).collect::<Vec<_>>(), ["base"]);
    assert!(app
        .resolver
        .catalog
        .asynchronous_sources()
        .unwrap()
        .is_empty());
}

#[test]
fn duplicate_source_rejection_preserves_catalog_and_never_persists() {
    let mut app = Application::new(
        Arc::new(QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.register_candidate_source(Arc::new(EmptySource("addon.async")));
    let registry = app.resolver.base_registry();
    let policy = app.resolver.catalog.policy();
    let version = app.resolver.catalog.version();
    let id = app.begin_activation().unwrap();
    let ticket = app.resolver.execution(id, 0);
    let persisted = AtomicUsize::new(0);
    let result = app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet {
            asynchronous: vec![Arc::new(PendingSource {
                started: Default::default(),
                ticket: Default::default(),
                resets: AtomicUsize::new(0),
            })],
            ..Default::default()
        },
        policy.clone(),
        || {
            persisted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );
    assert!(result.unwrap_err().to_string().contains("duplicate"));
    assert_eq!(persisted.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(&registry, &app.resolver.base_registry()));
    assert!(app.resolver.catalog.is_policy_current(&policy));
    assert_eq!(app.resolver.catalog.version(), version);
    assert!(ticket.is_current());
}

struct HeldSnapshot {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    signal: std::sync::Condvar,
}
impl HeldSnapshot {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.signal.notify_all();
    }
}
impl CandidateSource for HeldSnapshot {
    fn id(&self) -> &'static str {
        "held.snapshot"
    }
    fn candidates(&self, _: &SearchContext<'_>) -> Vec<crate::core::search::candidate::Candidate> {
        self.entered.notify_one();
        let (_released, timeout) = self
            .signal
            .wait_timeout_while(
                self.released.lock().unwrap(),
                Duration::from_secs(5),
                |released| !*released,
            )
            .unwrap();
        assert!(!timeout.timed_out(), "snapshot gate was not released");
        vec![]
    }
}

#[tokio::test]
async fn same_policy_catalog_publication_rejects_an_in_flight_snapshot() {
    let mut app = Application::new(
        Arc::new(QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let source = Arc::new(HeldSnapshot {
        entered: Default::default(),
        released: Mutex::new(false),
        signal: Default::default(),
    });
    app.register_candidate_source(source.clone());
    let app = Arc::new(app);
    let id = app.begin_activation().unwrap();
    let policy = app.resolver.catalog.policy();
    let pending = tokio::task::spawn_blocking({
        let app = app.clone();
        move || {
            app.resolver
                .commit(id, 0, vec![], vec![], true, std::time::Instant::now())
        }
    });
    let entered = tokio::time::timeout(Duration::from_secs(2), source.entered.notified()).await;
    if entered.is_err() {
        source.release();
    }
    entered.unwrap();
    let published = app.publish_feature_catalog(
        ActionRegistry::default(),
        ContextProviderRegistry::default(),
        SourceSet::default(),
        policy.clone(),
        || Ok(()),
    );
    source.release();
    published.unwrap();
    assert!(pending.await.unwrap().unwrap().is_none());
    assert!(app.resolver.catalog.is_policy_current(&policy));
    assert!(!app.is_session_active(id));
    assert!(app.latest_state(id).is_none());
}
