use super::*;
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, Platform};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
struct Clipboard {
    generation: AtomicI64,
    reads: AtomicUsize,
    payload: Option<anywhere_platform::clipboard::ClipboardPayload>,
    written: Mutex<Vec<anywhere_platform::clipboard::ClipboardPayload>>,
    on_read: Option<Box<dyn Fn() + Send + Sync>>,
    on_access: Option<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    read_error: Option<ContextReadError>,
    generation_error: bool,
}
impl Platform for Clipboard {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn open_url(&self, _: &str) -> Result<()> {
        unreachable!()
    }
    fn open_file(&self, _: &Path) -> Result<()> {
        unreachable!()
    }
    fn reveal_file(&self, _: &Path) -> Result<()> {
        unreachable!()
    }
    fn open_directory(&self, _: &Path) -> Result<()> {
        unreachable!()
    }
    fn open_terminal(&self, _: &Path) -> Result<()> {
        unreachable!()
    }
    fn copy_text(&self, _: &str) -> Result<()> {
        self.reads.fetch_add(100, Ordering::Relaxed);
        Ok(())
    }
    fn write_clipboard(
        &self,
        payload: &anywhere_platform::clipboard::ClipboardPayload,
    ) -> Result<()> {
        self.written.lock().unwrap().push(payload.clone());
        Ok(())
    }
    fn clipboard_generation(&self) -> Result<i64> {
        if self.generation_error {
            return Err(ContextReadError::Unavailable.into());
        }
        Ok(self.generation.load(Ordering::Relaxed))
    }
    fn request_clipboard_history_access(&self) -> Result<()> {
        self.on_access
            .as_ref()
            .map_or(Ok(()), |callback| callback())
    }
    fn clipboard_history_payload(
        &self,
        generation: i64,
    ) -> Result<Option<anywhere_platform::clipboard::ClipboardPayload>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if let Some(callback) = &self.on_read {
            callback();
        }
        if let Some(error) = self.read_error {
            return Err(error.into());
        }
        Ok(Some(self.payload.clone().unwrap_or_else(|| {
            anywhere_platform::clipboard::ClipboardPayload::text(&format!(
                "generation {generation}"
            ))
        })))
    }
}

#[test]
fn failed_resume_metadata_does_not_block_startup_or_import_old_content() {
    let (_directory, history, now) = timed_service();
    history
        .request_recording(true, &Clipboard::default())
        .unwrap();
    history.pause(5, &Clipboard::default()).unwrap();
    now.fetch_add(300_000, Ordering::SeqCst);
    let platform = Arc::new(Clipboard {
        generation_error: true,
        ..Default::default()
    });
    history.start(platform.clone()).unwrap();
    assert!(history.status().unwrap().available);
    assert!(!history.status().unwrap().recording);
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
}
fn service() -> (tempfile::TempDir, Arc<HistoryService>) {
    let directory = tempfile::tempdir().unwrap();
    let repository = crate::persistence::clipboard::SqliteHistoryRepository::open(
        &directory.path().join("clipboard.sqlite3"),
    )
    .unwrap();
    (
        directory,
        Arc::new(HistoryService::new(Ok(Box::new(repository)))),
    )
}

#[test]
fn disabled_feature_reports_capture_inactive_without_losing_recording_preference() {
    let (_directory, history) = service();
    let platform = Clipboard::default();
    history.request_recording(true, &platform).unwrap();
    assert!(history.status().unwrap().recording);

    history.set_feature_enabled(false, &platform).unwrap();
    let disabled = history.status().unwrap();
    assert!(!disabled.available && !disabled.recording);
    assert!(disabled.recording_enabled);
    platform.generation.store(1, Ordering::Relaxed);
    history.poll(&platform, 1).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);

    history.set_feature_enabled(true, &platform).unwrap();
    let enabled = history.status().unwrap();
    assert!(enabled.available && enabled.recording && enabled.recording_enabled);
    history.poll(&platform, 2).unwrap();
    assert!(history.search("").unwrap().is_empty());
    platform.generation.store(2, Ordering::Relaxed);
    history.poll(&platform, 3).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 1);
    assert_eq!(history.search("").unwrap().len(), 1);
}

fn timed_service() -> (tempfile::TempDir, Arc<HistoryService>, Arc<AtomicI64>) {
    let directory = tempfile::tempdir().unwrap();
    let repository = crate::persistence::clipboard::SqliteHistoryRepository::open(
        &directory.path().join("clipboard.sqlite3"),
    )
    .unwrap();
    let now = Arc::new(AtomicI64::new(10 * super::policy::DAY_MS));
    let clock = now.clone();
    let service = Arc::new(HistoryService::with_clock(
        Ok(Box::new(repository)),
        Arc::new(move || clock.load(Ordering::SeqCst)),
    ));
    (directory, service, now)
}

#[test]
fn background_resume_needs_no_open_window_and_drop_releases_the_worker() {
    let (_directory, history, now) = timed_service();
    let platform = Arc::new(Clipboard::default());
    history.request_recording(true, platform.as_ref()).unwrap();
    history.pause(5, platform.as_ref()).unwrap();
    history.start(platform.clone()).unwrap();
    now.fetch_add(300_000, Ordering::SeqCst);
    // The injected wall clock advanced without waiting five real minutes.
    history.wake_maintenance();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !history.status().unwrap().recording {
        assert!(
            std::time::Instant::now() < deadline,
            "background resume did not run"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    let weak = Arc::downgrade(&history);
    let platform_weak = Arc::downgrade(&platform);
    drop(platform);
    drop(history);
    assert!(weak.upgrade().is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while platform_weak.upgrade().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "worker retained the platform"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn timed_pause_survives_restart_and_resumes_without_importing_paused_copies() {
    let (directory, history, now) = timed_service();
    let platform = Clipboard::default();
    history.request_recording(true, &platform).unwrap();
    history.pause(5, &platform).unwrap();
    let deadline = now.load(Ordering::SeqCst) + 300_000;
    let status = history.status().unwrap();
    assert!(status.recording_enabled && !status.recording);
    assert_eq!(status.policy.paused_until_ms, Some(deadline));
    drop(history);
    let repository = crate::persistence::clipboard::SqliteHistoryRepository::open(
        &directory.path().join("clipboard.sqlite3"),
    )
    .unwrap();
    let clock = now.clone();
    let history = HistoryService::with_clock(
        Ok(Box::new(repository)),
        Arc::new(move || clock.load(Ordering::SeqCst)),
    );
    platform.generation.store(1, Ordering::Relaxed);
    history.poll(&platform, deadline - 1).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    now.store(deadline, Ordering::SeqCst);
    history.maintain(&platform).unwrap();
    history.poll(&platform, deadline).unwrap();
    assert!(history.status().unwrap().recording);
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    platform.generation.store(2, Ordering::Relaxed);
    history.poll(&platform, deadline + 1).unwrap();
    assert_eq!(history.search("").unwrap()[0].summary, "generation 2");
}

#[test]
fn pause_revokes_a_late_native_read_and_explicit_resume_excludes_existing_content() {
    let (_directory, history, now) = timed_service();
    let target = history.clone();
    let platform = Clipboard {
        on_read: Some(Box::new(move || {
            target.pause(15, &Clipboard::default()).unwrap()
        })),
        ..Default::default()
    };
    history.request_recording(true, &platform).unwrap();
    platform.generation.store(1, Ordering::Relaxed);
    history.poll(&platform, now.load(Ordering::SeqCst)).unwrap();
    assert!(history.search("").unwrap().is_empty());
    history.pause(0, &platform).unwrap();
    history.poll(&platform, now.load(Ordering::SeqCst)).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 1);
    history.pause(60, &platform).unwrap();
    history.request_recording(false, &platform).unwrap();
    assert!(history.status().unwrap().policy.paused_until_ms.is_none());
    now.fetch_add(3_600_000, Ordering::SeqCst);
    history.maintain(&platform).unwrap();
    assert!(!history.status().unwrap().recording_enabled);
}

#[test]
fn expired_history_cannot_be_searched_previewed_or_copied_and_formats_are_deleted() {
    let (directory, history, now) = timed_service();
    let platform = Clipboard::default();
    history.request_recording(true, &platform).unwrap();
    history.set_retention_days(1).unwrap();
    platform.generation.store(1, Ordering::Relaxed);
    history.poll(&platform, now.load(Ordering::SeqCst)).unwrap();
    let id = history.search("").unwrap()[0].id;
    now.fetch_add(super::policy::DAY_MS, Ordering::SeqCst);
    assert!(history.detail(id).unwrap().is_none());
    assert!(history.copy(id, &platform).is_err());
    assert!(history.search("").unwrap().is_empty());
    assert!(platform.written.lock().unwrap().is_empty());
    assert_eq!(history.usage().unwrap(), (0, 0));
    let connection =
        rusqlite::Connection::open(directory.path().join("clipboard.sqlite3")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM clipboard_formats", [], |r| r
                .get::<_, usize>(0))
            .unwrap(),
        0
    );
}

#[test]
fn expiry_or_policy_change_during_preview_never_publishes_the_old_body() {
    for change_policy in [false, true] {
        let (_directory, history, now) = timed_service();
        let platform = Clipboard::default();
        history.request_recording(true, &platform).unwrap();
        history.set_retention_days(1).unwrap();
        platform.generation.store(1, Ordering::Relaxed);
        history.poll(&platform, now.load(Ordering::SeqCst)).unwrap();
        let id = history.search("").unwrap()[0].id;
        let result = history
            .detail_with(id, |item, payload| {
                if change_policy {
                    history.set_retention_days(7).unwrap();
                } else {
                    now.fetch_add(super::policy::DAY_MS, Ordering::SeqCst);
                }
                super::super::content::preview(item, payload)
            })
            .unwrap();
        assert!(result.is_none());
    }
}

#[test]
fn shortening_retention_is_immediate_and_expanding_it_does_not_restore_deleted_entries() {
    let (_directory, history, now) = timed_service();
    let platform = Clipboard::default();
    history.request_recording(true, &platform).unwrap();
    platform.generation.store(1, Ordering::Relaxed);
    history.poll(&platform, now.load(Ordering::SeqCst)).unwrap();
    now.fetch_add(2 * super::policy::DAY_MS, Ordering::SeqCst);
    assert_eq!(history.search("").unwrap().len(), 1);
    history.set_retention_days(1).unwrap();
    assert!(history.search("").unwrap().is_empty());
    history.set_retention_days(0).unwrap();
    assert!(history.search("").unwrap().is_empty());
    assert!(history.pause(2, &platform).is_err());
    assert!(history.set_retention_days(u32::MAX).is_err());
    assert_eq!(history.status().unwrap().policy.retention_days, 0);
}
#[test]
fn status_exposes_policy_facts_for_disabled_storage_permission_and_capture_failures() {
    let broken = HistoryService::new(Err("private storage diagnostic".into()));
    assert_eq!(
        broken.status().unwrap().notice,
        Some(HistoryNotice::StorageUnavailable)
    );
    let (_directory, history) = service();
    let platform = Clipboard {
        on_access: Some(Box::new(|| Err(ContextReadError::PermissionDenied.into()))),
        ..Default::default()
    };
    history.request_recording(true, &platform).unwrap();
    assert_eq!(
        history.status().unwrap().notice,
        Some(HistoryNotice::AccessRequired)
    );
    history.set_feature_enabled(false, &platform).unwrap();
    assert_eq!(
        history.status().unwrap().notice,
        Some(HistoryNotice::FeatureDisabled)
    );
    for (error, expected) in [
        (ContextReadError::TooLarge, HistoryNotice::ItemTooLarge),
        (
            ContextReadError::PermissionDenied,
            HistoryNotice::AccessRequired,
        ),
        (ContextReadError::Unavailable, HistoryNotice::ReadFailed),
    ] {
        let (_directory, history) = service();
        let platform = Clipboard {
            read_error: Some(error),
            ..Default::default()
        };
        history.request_recording(true, &platform).unwrap();
        platform.generation.store(1, Ordering::Relaxed);
        history.poll(&platform, 1).unwrap();
        let status = history.status().unwrap();
        assert!(status.available && status.recording);
        assert_eq!(status.notice, Some(expected));
        assert!(history.search("").unwrap().is_empty());
    }
}

#[test]
fn only_new_copies_are_recorded_and_pause_resume_never_imports_old_contents() {
    let (_directory, service) = service();
    let platform = Clipboard::default();
    service.poll(&platform, 0).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    service.set_recording(true).unwrap();
    service.poll(&platform, 1).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    platform.generation.store(1, Ordering::Relaxed);
    service.poll(&platform, 2).unwrap();
    service.poll(&platform, 3).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 1);
    assert_eq!(service.search("").unwrap().len(), 1);
    service.set_recording(false).unwrap();
    platform.generation.store(2, Ordering::Relaxed);
    service.poll(&platform, 4).unwrap();
    service.set_recording(true).unwrap();
    service.poll(&platform, 5).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 1);
    platform.generation.store(3, Ordering::Relaxed);
    service.poll(&platform, 6).unwrap();
    assert_eq!(service.search("").unwrap()[0].summary, "generation 3");
    let id = service.search("").unwrap()[0].id;
    service.delete(id).unwrap();
    assert!(service.copy(id, &platform).is_err());
    assert_eq!(platform.reads.load(Ordering::Relaxed), 2);
    service.clear().unwrap();
    service.poll(&platform, 7).unwrap();
    assert!(service.search("").unwrap().is_empty());
}
#[test]
fn enable_captures_the_first_copy_without_waiting_for_a_baseline_tick() {
    let (_directory, service) = service();
    let platform = Clipboard::default();
    platform.generation.store(10, Ordering::Relaxed);
    service.request_recording(true, &platform).unwrap();
    assert_eq!(platform.reads.load(Ordering::Relaxed), 0);
    platform.generation.store(11, Ordering::Relaxed);
    service.poll(&platform, 1).unwrap();
    assert_eq!(service.search("").unwrap()[0].summary, "generation 11");
    service.request_recording(false, &platform).unwrap();
    platform.generation.store(12, Ordering::Relaxed);
    service.request_recording(true, &platform).unwrap();
    service.poll(&platform, 2).unwrap();
    assert_eq!(service.search("").unwrap().len(), 1);
}

#[test]
fn permission_response_cannot_restore_a_notice_after_recording_is_paused() {
    let (_directory, service) = service();
    let target = service.clone();
    let platform = Clipboard {
        on_access: Some(Box::new(move || {
            target.set_recording(false)?;
            Err(anywhere_platform::ContextReadError::PermissionDenied.into())
        })),
        ..Default::default()
    };
    service.request_recording(true, &platform).unwrap();
    let status = service.status().unwrap();
    assert!(!status.recording);
    assert!(status.notice.is_none());
}

#[test]
fn module_resume_excludes_paused_copies_and_keeps_the_first_new_copy() {
    let (_directory, service) = service();
    let platform = Clipboard::default();
    service.request_recording(true, &platform).unwrap();
    service.set_feature_enabled(false, &platform).unwrap();
    platform.generation.store(10, Ordering::Relaxed);
    service.set_feature_enabled(true, &platform).unwrap();
    platform.generation.store(11, Ordering::Relaxed);
    service.poll(&platform, 1).unwrap();
    let rows = service.search("").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].summary, "generation 11");
}

#[test]
fn disable_or_clear_during_native_read_revokes_the_pending_write() {
    for disable in [false, true] {
        let (_directory, service) = service();
        service.set_recording(true).unwrap();
        let callback_service = service.clone();
        let platform = Clipboard {
            on_read: Some(Box::new(move || {
                if disable {
                    callback_service
                        .set_feature_enabled(false, &Clipboard::default())
                        .unwrap();
                } else {
                    callback_service.clear().unwrap();
                }
            })),
            ..Default::default()
        };
        service.poll(&platform, 0).unwrap();
        platform.generation.store(1, Ordering::Relaxed);
        service.poll(&platform, 1).unwrap();
        if disable {
            assert!(service.search("").is_err());
            assert!(service.detail(1).is_err());
            assert!(service.copy(1, &platform).is_err());
            assert!(service.set_recording(true).is_err());
            service.set_feature_enabled(true, &platform).unwrap();
        }
        assert!(service.search("").unwrap().is_empty());
    }
}

#[test]
fn captured_multiformat_item_restores_all_original_formats() {
    use anywhere_platform::clipboard::{ClipboardFormat, ClipboardPayload};
    let (_directory, service) = service();
    let payload = ClipboardPayload {
        formats: vec![
            ClipboardFormat {
                name: "public.png".into(),
                data: vec![137, 80, 78, 71],
            },
            ClipboardFormat {
                name: "com.example.binary".into(),
                data: vec![0, 255, 1],
            },
        ],
    };
    let platform = Clipboard {
        payload: Some(payload.clone()),
        ..Default::default()
    };
    service.request_recording(true, &platform).unwrap();
    platform.generation.store(1, Ordering::Relaxed);
    service.poll(&platform, 1).unwrap();
    let item = service.search("").unwrap().remove(0);
    assert_eq!(item.kind, "image");
    let preview = service.detail(item.id).unwrap().unwrap();
    assert_eq!(preview.formats.len(), 2);
    service.copy(item.id, &platform).unwrap();
    let written = platform.written.lock().unwrap();
    assert_eq!(written.len(), 1);
    for format in payload.formats {
        assert!(written[0].formats.contains(&format));
    }
}

#[test]
fn deletion_clear_and_disable_during_preview_revoke_its_result() {
    for operation in ["delete", "clear", "disable"] {
        let (_directory, service) = service();
        let platform = Clipboard::default();
        service.request_recording(true, &platform).unwrap();
        platform.generation.store(1, Ordering::Relaxed);
        service.poll(&platform, 1).unwrap();
        let id = service.search("").unwrap()[0].id;
        let result = service.detail_with(id, |item, payload| {
            match operation {
                "delete" => service.delete(id).unwrap(),
                "clear" => service.clear().unwrap(),
                _ => service.set_feature_enabled(false, &platform).unwrap(),
            }
            super::super::content::preview(item, payload)
        });
        if operation == "disable" {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_none());
        }
    }
}

#[test]
fn preview_admission_serializes_bodies_without_blocking_policy_or_storage() {
    let (_directory, service) = service();
    let platform = Clipboard::default();
    service.request_recording(true, &platform).unwrap();
    platform.generation.store(1, Ordering::Relaxed);
    service.poll(&platform, 1).unwrap();
    let id = service.search("").unwrap()[0].id;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_service = service.clone();
    let first = std::thread::spawn(move || {
        first_service.detail_with(id, |item, payload| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            super::super::content::preview(item, payload)
        })
    });
    entered_rx.recv().unwrap();
    assert!(service.preview.try_lock().is_err());
    assert_eq!(service.search("").unwrap().len(), 1);
    service.set_recording(false).unwrap();
    service.delete(id).unwrap();
    let second_service = service.clone();
    let second = std::thread::spawn(move || second_service.detail(id));
    release_tx.send(()).unwrap();
    assert!(first.join().unwrap().unwrap().is_none());
    assert!(second.join().unwrap().unwrap().is_none());
}
