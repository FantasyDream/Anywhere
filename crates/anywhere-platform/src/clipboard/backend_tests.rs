use super::*;
use clipboard_rs::{common::RustImageData, ContentFormat};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "macos")]
#[test]
fn cocoa_exception_is_an_error_and_does_not_abort_the_process() {
    use objc2::{msg_send, rc::Retained, runtime::NSObject};
    let result = native_operation(|| {
        let object = NSObject::new();
        // SAFETY: deliberately send an unsupported NSObject selector to raise
        // a real Objective-C exception inside the production catch boundary.
        let _: Retained<NSObject> = unsafe { msg_send![&*object, copy] };
        Ok(())
    });
    assert!(result
        .unwrap_err()
        .downcast_ref::<ContextReadError>()
        .is_some());
    assert_eq!(native_operation(|| Ok(42)).unwrap(), 42);
}
#[derive(Default)]
struct Board {
    formats: Vec<(String, Vec<u8>)>,
    text: Option<String>,
    files: Option<Vec<String>>,
    reads: AtomicUsize,
}
impl Clipboard for Board {
    fn available_formats(&self) -> clipboard_rs::Result<Vec<String>> {
        Ok(self.formats.iter().map(|f| f.0.clone()).collect())
    }
    fn has(&self, f: ContentFormat) -> bool {
        match f {
            ContentFormat::Text => self.text.is_some(),
            ContentFormat::Files => self.files.is_some(),
            _ => false,
        }
    }
    fn get_buffer(&self, f: &str) -> clipboard_rs::Result<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.formats
            .iter()
            .find(|v| v.0 == f)
            .map(|v| v.1.clone())
            .ok_or_else(|| "missing".into())
    }
    fn get_text(&self) -> clipboard_rs::Result<String> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.text.clone().ok_or_else(|| "missing".into())
    }
    fn get_files(&self) -> clipboard_rs::Result<Vec<String>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.files.clone().ok_or_else(|| "missing".into())
    }
    fn clear(&self) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn get_rich_text(&self) -> clipboard_rs::Result<String> {
        unreachable!()
    }
    fn get_html(&self) -> clipboard_rs::Result<String> {
        unreachable!()
    }
    fn get_image(&self) -> clipboard_rs::Result<RustImageData> {
        unreachable!()
    }
    fn get(&self, _: &[ContentFormat]) -> clipboard_rs::Result<Vec<ClipboardContent>> {
        unreachable!()
    }
    fn set_buffer(&self, _: &str, _: Vec<u8>) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set_text(&self, _: String) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set_rich_text(&self, _: String) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set_html(&self, _: String) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set_image(&self, _: RustImageData) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set_files(&self, _: Vec<String>) -> clipboard_rs::Result<()> {
        unreachable!()
    }
    fn set(&self, _: Vec<ClipboardContent>) -> clipboard_rs::Result<()> {
        unreachable!()
    }
}
#[test]
fn capture_preserves_raw_image_rtf_html_and_custom_data_without_decoding() {
    let board = Board {
        formats: vec![
            ("public.utf8-plain-text".into(), b"Hello".to_vec()),
            ("public.html".into(), b"<b>Hello</b>".to_vec()),
            ("public.rtf".into(), vec![0, 255, 1]),
            ("public.png".into(), vec![137, 80, 78, 71]),
            ("com.example.binary".into(), vec![0, 1, 2, 255]),
        ],
        text: Some("Hello".into()),
        ..Default::default()
    };
    let payload = collect(&board, |_, _| Ok(true), || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(payload.plain_text(), Some("Hello"));
    assert_eq!(payload.formats.len(), 5);
    assert_eq!(
        payload
            .formats
            .iter()
            .find(|f| f.name == "public.rtf")
            .unwrap()
            .data,
        vec![0, 255, 1]
    );
    assert_eq!(
        payload
            .formats
            .iter()
            .find(|f| f.name == "com.example.binary")
            .unwrap()
            .data,
        vec![0, 1, 2, 255]
    );
}
#[test]
fn sensitive_markers_and_size_preflight_prevent_content_reads() {
    let mut board = Board {
        formats: vec![("org.nspasteboard.ConcealedType".into(), vec![])],
        text: Some("private".into()),
        ..Default::default()
    };
    assert!(collect(&board, |_, _| unreachable!(), || unreachable!())
        .unwrap()
        .is_none());
    assert_eq!(board.reads.load(Ordering::Relaxed), 0);
    board.formats = vec![("public.png".into(), vec![0])];
    assert!(collect(
        &board,
        |_, _| Err(ContextReadError::TooLarge.into()),
        || Ok(())
    )
    .is_err());
    assert_eq!(board.reads.load(Ordering::Relaxed), 0);
}
#[test]
fn file_capture_keeps_all_paths_and_drops_finder_cut_metadata() {
    let board = Board {
        formats: vec![
            ("public.file-url".into(), vec![]),
            ("com.apple.finder.cut".into(), vec![1]),
        ],
        files: Some(vec!["/tmp/a file".into(), "/tmp/中文".into()]),
        ..Default::default()
    };
    let payload = collect(&board, |_, _| unreachable!(), || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(payload.files().unwrap(), board.files.unwrap());
    assert_eq!(payload.formats.len(), 1);
}
#[test]
fn unavailable_data_and_promises_do_not_become_fake_history_entries() {
    let board = Board {
        formats: vec![
            ("com.apple.pasteboard.promised-file-url".into(), vec![]),
            ("public.png".into(), vec![1]),
        ],
        ..Default::default()
    };
    assert!(collect(&board, |_, _| Ok(false), || Ok(()))
        .unwrap()
        .is_none());
    assert_eq!(board.reads.load(Ordering::Relaxed), 0);
    let files = ClipboardPayload {
        formats: vec![ClipboardFormat {
            name: FILES.into(),
            data: serde_json::to_vec(&vec!["/this-is-an-anywhere-test-missing-file/xyz"]).unwrap(),
        }],
    };
    // All paths are checked before creating a native clipboard context or clearing it.
    assert!(write(&files).is_err());
}

#[test]
fn macos_plain_text_aliases_do_not_create_another_identity_after_restore() {
    let board = Board {
        formats: vec![
            ("public.utf8-plain-text".into(), b"same text".to_vec()),
            ("NSStringPboardType".into(), b"same text".to_vec()),
            (
                "public.utf16-external-plain-text".into(),
                vec![255, 254, 115, 0],
            ),
            ("CorePasteboardFlavorType 0x75743136".into(), vec![115, 0]),
        ],
        text: Some("same text".into()),
        ..Default::default()
    };
    let payload = collect(&board, |_, _| Ok(true), || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(payload, ClipboardPayload::text("same text"));
    let mut rich = board;
    rich.formats
        .push(("public.rtf".into(), b"{\\rtf1 same text}".to_vec()));
    let payload = collect(&rich, |_, _| Ok(true), || Ok(())).unwrap().unwrap();
    assert_eq!(payload.formats.len(), 2);
    assert!(payload.formats.iter().any(|f| f.name == "public.rtf"));
}
