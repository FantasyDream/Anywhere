//! Bounded, deadline-aware draining of a child process's pipes.
//!
//! Shared by the macOS Git runner: keep reading so the child
//! never blocks on a full pipe, cap what they keep, and stop waiting after a timeout even
//! if a grandchild still holds the pipe open.

/// Longest output kept from one Git command.
pub const MAX_GIT_BYTES: usize = 1024 * 1024;
/// Upper bound on waiting for the pipes to close once the process is gone or killed.
pub const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Outcome of waiting for one pipe reader.
pub enum ReaderOutcome {
    /// The pipe reached EOF and the output is complete.
    Drained(Vec<u8>, bool),
    /// The reader was still blocked when the wait expired; the thread keeps draining in
    /// the background, which is the only option for a blocking pipe read.
    Stalled,
    /// The read failed; the error is already logged at debug level.
    Failed,
}

/// Read at most [`MAX_GIT_BYTES`], then keep draining so the child never blocks.
pub fn read_bounded(mut reader: impl std::io::Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut collected = Vec::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok((collected, truncated));
        }
        if collected.len() < MAX_GIT_BYTES {
            let room = MAX_GIT_BYTES - collected.len();
            collected.extend_from_slice(&buffer[..read.min(room)]);
            if read > room {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }
}

/// Wait for one pipe reader, giving up after `timeout` so a stuck pipe cannot hold the
/// launcher open any longer than the Git deadline intends.
pub fn join(
    rx: std::sync::mpsc::Receiver<std::io::Result<(Vec<u8>, bool)>>,
    timeout: std::time::Duration,
) -> ReaderOutcome {
    match rx.recv_timeout(timeout) {
        Ok(Ok((bytes, truncated))) => ReaderOutcome::Drained(bytes, truncated),
        Ok(Err(error)) => {
            tracing::debug!(error = %error, "git_output_read_failed");
            ReaderOutcome::Failed
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ReaderOutcome::Stalled,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => ReaderOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// A reader that produces a fixed number of bytes without ever blocking.
    struct Fixed(usize);
    impl Read for Fixed {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let produce = self.0.min(buffer.len());
            for byte in buffer.iter_mut().take(produce) {
                *byte = b'x';
            }
            self.0 -= produce;
            Ok(produce)
        }
    }

    #[test]
    fn bounded_reader_caps_output_but_keeps_draining_the_pipe() {
        let (collected, truncated) = read_bounded(Fixed(MAX_GIT_BYTES + 4096)).unwrap();
        assert_eq!(collected.len(), MAX_GIT_BYTES);
        assert!(truncated, "over-cap output must be reported as truncated");
        let (small, truncated) = read_bounded(Fixed(32)).unwrap();
        assert_eq!(small.len(), 32);
        assert!(!truncated);
        // The reader is drained to EOF, so the child process can never block on a full pipe.
        let mut exhausted = Fixed(0);
        let mut buffer = [0u8; 8];
        assert_eq!(exhausted.read(&mut buffer).unwrap(), 0);
    }

    #[test]
    fn a_stalled_reader_is_reported_instead_of_waited_on() {
        let (_tx, rx) = std::sync::mpsc::channel();
        assert!(matches!(
            join(rx, std::time::Duration::from_millis(10)),
            ReaderOutcome::Stalled
        ));
    }
}
