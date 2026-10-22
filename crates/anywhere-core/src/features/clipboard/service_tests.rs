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
