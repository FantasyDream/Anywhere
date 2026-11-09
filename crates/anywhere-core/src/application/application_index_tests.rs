use super::{source_catalog_tests::QuietPlatform, Application};
use crate::{
    core::{
        action::descriptor::ActionRegistry,
        ranking::usage::MemoryUsage,
        search::{
            candidate::Candidate,
            candidate_source::{CandidateSource, SearchContext},
        },
    },
    features::applications::index::{ApplicationCatalog, IndexedApplication},
};
use anywhere_protocol::{CandidateKind, InputMode};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Gate {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    fn wait(&self) {
        self.entered.notify_one();
        let (released, _) = self
            .changed
            .wait_timeout_while(self.released.lock().unwrap(), Duration::from_secs(5), |r| {
                !*r
            })
            .unwrap();
        assert!(*released, "test did not release snapshot preparation");
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

#[derive(Default)]
struct HeldSearch {
    armed: AtomicBool,
    calls: AtomicUsize,
    old: Gate,
    updated: Gate,
}
impl CandidateSource for HeldSearch {
    fn candidates(&self, _: &SearchContext<'_>) -> Vec<Candidate> {
        if self.armed.load(Ordering::Acquire) {
            match self.calls.fetch_add(1, Ordering::AcqRel) {
                0 => self.old.wait(),
                1 => self.updated.wait(),
                _ => {}
            }
        }
        vec![]
    }
}

#[tokio::test]
async fn exclusion_changes_reject_prepared_rows_before_the_replacement_snapshot_finishes() {
    let source = Arc::new(HeldSearch::default());
    let mut app = Application::new(
        Arc::new(QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.register_candidate_source(source.clone());
    let id = app.begin_activation().unwrap();
    {
        let mut catalog = app.resolver.catalog.lock().unwrap();
        catalog.full = Arc::new(ApplicationCatalog::new(vec![IndexedApplication {
            bundle_id: "dev.test.index".into(),
            name: "Index".into(),
            path: "/synthetic/Index.app".into(),
            ..Default::default()
        }
        .prepared()]));
        catalog.catalog = catalog.full.clone();
        let mut session = app.session().unwrap();
        session.input_mode = InputMode::Search;
        session.state.input = "Index".into();
        session.resolution.committed_revision = Some(0);
    }
    let initial = app.resolver.recompute_current(id).unwrap();
    assert!(initial
        .candidates
        .iter()
        .any(|c| c.kind == CandidateKind::Application));
    let app = Arc::new(app);
    source.armed.store(true, Ordering::Release);
    let old = tokio::task::spawn_blocking({
        let app = app.clone();
        move || app.resolver.recompute_current(id)
    });
    let old_started =
        tokio::time::timeout(Duration::from_secs(2), source.old.entered.notified()).await;
    if old_started.is_err() {
        source.old.release();
        source.updated.release();
    }
    old_started.unwrap();
    // Keep the replacement calculation pending so a snapshot-sequence change
    // cannot incidentally protect the old calculation from the changed index.
    let update = tokio::task::spawn_blocking({
        let app = app.clone();
        move || app.set_excluded_apps(vec!["dev.test.index".into()])
    });
    let update_started =
        tokio::time::timeout(Duration::from_secs(2), source.updated.entered.notified()).await;
    source.old.release();
    let completed = tokio::time::timeout(Duration::from_secs(2), old).await;
    source.updated.release();
    update.await.unwrap();
    update_started.unwrap();
    let state = completed.unwrap().unwrap().unwrap();
    assert!(
        state
            .candidates
            .iter()
            .all(|c| c.kind != CandidateKind::Application),
        "old preparation published an excluded application: {:?}",
        state.candidates
    );
    assert_eq!(app.excluded_apps(), ["dev.test.index"]);
}
