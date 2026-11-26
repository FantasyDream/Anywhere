use super::*;
use crate::application::{source_catalog_tests::QuietPlatform, Application};
use crate::{
    core::{
        action::descriptor::ActionRegistry,
        ranking::usage::MemoryUsage,
        search::{
            candidate::Candidate,
            candidate_source::{CandidateSource, SearchContext},
        },
    },
    features::applications::index::{IndexedApplication, ScanReport},
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
struct CatalogProbe {
    calls: AtomicUsize,
    gate: Option<Arc<Gate>>,
}

impl ApplicationCatalogHost for CatalogProbe {
    fn installed_applications(&self) -> anywhere_platform::InstalledApplications {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        anywhere_platform::InstalledApplications::default()
    }
}

#[tokio::test]
async fn activation_bursts_keep_only_one_running_scan_and_the_latest_follow_up() {
    let index = Arc::new(ApplicationIndex::default());
    let first_gate = Arc::new(Gate::default());
    let first = Arc::new(CatalogProbe {
        gate: Some(first_gate.clone()),
        ..Default::default()
    });
    let skipped = Arc::new(CatalogProbe::default());
    let last_gate = Arc::new(Gate::default());
    let last = Arc::new(CatalogProbe {
        gate: Some(last_gate.clone()),
        ..Default::default()
    });

    let first_generation = index.spawn_scan(first.clone());
    tokio::time::timeout(Duration::from_secs(2), first_gate.entered.notified())
        .await
        .unwrap();
    for _ in 0..16 {
        index.spawn_scan(skipped.clone());
    }
    let last_generation = index.spawn_scan(last.clone());
    assert_eq!(last.calls.load(Ordering::Acquire), 0);
    first_gate.release();
    let started = tokio::time::timeout(Duration::from_secs(2), last_gate.entered.notified()).await;
    last_gate.release();
    started.unwrap();
    // The worker published the first scan before entering the last one. Wait
    // for the latest generation even though intermediate requests were merged.
    assert!(index.snapshot().unwrap().completed_scan >= first_generation);
    tokio::time::timeout(Duration::from_secs(2), index.ready())
        .await
        .unwrap();
    assert_eq!(index.snapshot().unwrap().completed_scan, last_generation);
    assert_eq!(first.calls.load(Ordering::Acquire), 1);
    assert_eq!(skipped.calls.load(Ordering::Acquire), 0);
    assert_eq!(last.calls.load(Ordering::Acquire), 1);
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
    app.resolver
        .application_index
        .publish(0, report(&["Index"], 0));
    {
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

fn report(names: &[&str], skipped: usize) -> ScanStatus {
    ScanStatus::Complete(ScanReport {
        applications: names
            .iter()
            .map(|name| {
                IndexedApplication {
                    bundle_id: format!("dev.test.{}", name.to_lowercase()),
                    name: (*name).into(),
                    path: format!("/synthetic/{name}.app"),
                    ..Default::default()
                }
                .prepared()
            })
            .collect(),
        skipped,
    })
}

fn names(snapshot: &IndexSnapshot) -> Vec<&str> {
    snapshot
        .catalog
        .applications()
        .iter()
        .map(|app| app.name.as_str())
        .collect()
}

#[test]
fn application_index_drops_a_scan_from_superseded_search_paths() {
    let index = ApplicationIndex::default();
    index.set_paths(vec!["old-source".into()]);
    let old_paths = index.published.lock().unwrap().paths.clone();
    let old_generation = index.issue_scan();
    index.publish_for_paths(old_generation, report(&["Old"], 0), Some(&old_paths));
    let old_snapshot = index.snapshot().unwrap();
    index.set_paths(vec!["new-source".into()]);
    assert!(!index.is_current(&old_snapshot).unwrap());
    assert!(names(&index.snapshot().unwrap()).is_empty());
    let new_paths = index.published.lock().unwrap().paths.clone();
    let new_generation = index.issue_scan();

    index.publish_for_paths(old_generation, report(&["Old"], 0), Some(&old_paths));
    assert!(names(&index.snapshot().unwrap()).is_empty());
    index.publish_for_paths(new_generation, ScanStatus::Failed, Some(&new_paths));
    assert!(names(&index.snapshot().unwrap()).is_empty());
    index.publish_for_paths(new_generation, report(&["New"], 0), Some(&new_paths));
    assert_eq!(names(&index.snapshot().unwrap()), ["New"]);
}

#[test]
fn an_older_scan_cannot_replace_a_newer_completed_index() {
    let index = ApplicationIndex::default();
    let older = index.issue_scan();
    let newer = index.issue_scan();
    index.publish(newer, report(&["New"], 0));
    let published = index.snapshot().unwrap();
    index.publish(older, report(&["Old"], 0));
    assert_eq!(names(&index.snapshot().unwrap()), ["New"]);
    assert!(index.is_current(&published).unwrap());
    assert_eq!(*index.updates().borrow(), newer);
}

#[tokio::test(start_paused = true)]
async fn readiness_waits_for_the_current_scan_and_failure_keeps_the_previous_index() {
    let index = ApplicationIndex::default();
    let first = index.issue_scan();
    index.publish(first, report(&["Existing"], 0));
    index.ready().await;
    let previous = index.snapshot().unwrap();
    let older = index.issue_scan();
    let newest = index.issue_scan();
    let waiting = index.ready();
    tokio::pin!(waiting);
    tokio::select! {
        biased;
        _ = &mut waiting => panic!("readiness reused an earlier completion"),
        _ = std::future::ready(()) => {}
    }
    index.publish(newest, ScanStatus::Failed);
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap();
    assert_eq!(*index.updates().borrow(), newest);
    let retained = index.snapshot().unwrap();
    assert!(Arc::ptr_eq(&previous.catalog, &retained.catalog));
    assert!(!index.is_current(&previous).unwrap());
    // Failure also seals its generation against a late successful older scan.
    index.publish(older, report(&["Obsolete"], 0));
    assert_eq!(names(&index.snapshot().unwrap()), ["Existing"]);
    assert!(index.is_current(&retained).unwrap());
}

#[test]
fn exclusions_filter_new_scans_without_removing_shortcut_configuration_labels() {
    let app = Application::new(
        Arc::new(QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let index = &app.resolver.application_index;
    let first = index.issue_scan();
    index.publish(first, report(&["Index", "Other"], 0));
    let original = index.snapshot().unwrap();
    app.set_excluded_apps(vec![
        "dev.test.index".into(),
        "".into(),
        "  ".into(),
        "dev.test.index".into(),
    ]);
    assert_eq!(app.excluded_apps(), ["dev.test.index"]);
    assert_eq!(names(&index.snapshot().unwrap()), ["Other"]);
    assert!(!index.is_current(&original).unwrap());
    assert_eq!(app.shortcut_applications().unwrap().len(), 2);
    let pending = index.issue_scan();
    app.set_excluded_apps(vec!["dev.test.fresh".into()]);
    index.publish(pending, report(&["Index", "Fresh"], 0));
    assert_eq!(names(&index.snapshot().unwrap()), ["Index"]);
    let full = index.full_catalog().unwrap();
    assert_eq!(full.applications().len(), 2);
    app.set_excluded_apps(vec![]);
    let restored = index.snapshot().unwrap();
    assert!(Arc::ptr_eq(&full, &restored.catalog));
    assert_eq!(names(&restored), ["Index", "Fresh"]);
}

#[test]
fn partial_and_empty_successes_replace_the_index_while_a_pending_scan_keeps_it() {
    let index = ApplicationIndex::default();
    let first = index.issue_scan();
    index.publish(first, report(&["Old"], 0));
    let previous = index.snapshot().unwrap();
    let next = index.issue_scan();
    assert!(index.is_current(&previous).unwrap());
    index.publish(next, report(&["Readable"], 1));
    assert!(!index.is_current(&previous).unwrap());
    assert_eq!(names(&index.snapshot().unwrap()), ["Readable"]);
    let empty = index.issue_scan();
    index.publish(empty, report(&[], 0));
    assert!(index.snapshot().unwrap().catalog.is_empty());
    assert!(index.full_catalog().unwrap().is_empty());
    assert_eq!(*index.updates().borrow(), empty);
}

#[test]
fn reapplying_the_same_exclusions_keeps_the_current_catalog() {
    let index = ApplicationIndex::default();
    index.publish(index.issue_scan(), report(&["Editor", "Terminal"], 0));
    assert!(index.set_excluded(vec!["dev.test.editor".into()]).unwrap());
    let snapshot = index.snapshot().unwrap();

    assert!(!index
        .set_excluded(vec!["dev.test.editor".into(), "dev.test.editor".into()])
        .unwrap());
    assert!(index.is_current(&snapshot).unwrap());
    assert_eq!(names(&snapshot), ["Terminal"]);
}
