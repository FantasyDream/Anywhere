//! Metadata-only polling; exceptions are contained by the native generation query.
use super::ClipboardWatch;
use crate::Platform;
use anyhow::Result;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

struct Watch(Sender<()>);
impl ClipboardWatch for Watch {}
impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn spawn(
    changed: Box<dyn Fn() + Send>,
    mut generation: impl FnMut() -> Option<i64> + Send + 'static,
    interval: Duration,
) -> Result<Box<dyn ClipboardWatch>> {
    let (stop, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || {
            let mut previous = generation();
            while matches!(
                receiver.recv_timeout(interval),
                Err(RecvTimeoutError::Timeout)
            ) {
                if let Some(current) = generation() {
                    let changed_generation = previous.is_some_and(|last| last != current);
                    previous = Some(current);
                    if changed_generation {
                        changed();
                    }
                } else {
                    // A recovered service establishes a fresh baseline; its
                    // existing content must not be reported as a new copy.
                    previous = None;
                }
            }
        })?;
    Ok(Box::new(Watch(stop)))
}

pub(super) fn watch(changed: Box<dyn Fn() + Send>) -> Result<Box<dyn ClipboardWatch>> {
    spawn(
        changed,
        // This metadata-only port also invalidates clipboard recency when the
        // pasteboard service fails. It never reads or requests content access.
        || crate::macos::MacOsPlatform.clipboard_generation().ok(),
        Duration::from_millis(500),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_queries_do_not_publish_changes_and_drop_stops_polling() {
        let (values, queries) = mpsc::channel();
        let (events, observed) = mpsc::channel();
        let (ended, finished) = mpsc::channel();
        struct End(Sender<()>);
        impl Drop for End {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let end = End(ended);
        let watch = spawn(
            Box::new(move || {
                events.send(()).unwrap();
            }),
            move || {
                let _ = &end;
                queries.recv().unwrap()
            },
            Duration::from_millis(1),
        )
        .unwrap();
        for value in [Some(0), None, Some(0), Some(1)] {
            values.send(value).unwrap();
        }
        observed.recv_timeout(Duration::from_secs(2)).unwrap();
        for value in [Some(1), None, Some(2), Some(2), Some(3)] {
            values.send(value).unwrap();
        }
        observed.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(observed.try_recv().is_err());
        drop(watch);
        // Unblock a read that may already be running. The next wait sees stop.
        let _ = values.send(Some(3));
        finished.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
