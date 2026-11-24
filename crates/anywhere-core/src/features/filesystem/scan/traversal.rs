//! Disk-backed FIFO traversal and flush-before-publication of complete records.
use super::{
    query::insert_top,
    record::{read_record, write_match_record, write_record},
    ScanPage, ScanStore, BUFFER, PAGE_SIZE,
};
use anyhow::{bail, Result};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Seek, SeekFrom, Write},
    sync::atomic::Ordering as AtomicOrdering,
    time::{Duration, Instant},
};

impl ScanStore {
    pub fn run(
        &self,
        cancelled: &(dyn Fn() -> bool + Sync),
        mut publish: impl FnMut(ScanPage) -> Result<()>,
    ) -> Result<()> {
        let queue_path = self.directory.path().join("pending-dirs");
        struct PendingDirsCleanup<'a> {
            store: &'a ScanStore,
            path: std::path::PathBuf,
        }
        impl Drop for PendingDirsCleanup<'_> {
            fn drop(&mut self) {
                let _ = self.store.remove_run(&self.path);
            }
        }
        let _queue_cleanup = PendingDirsCleanup {
            store: self,
            path: queue_path.clone(),
        };
        let mut queue = BufWriter::with_capacity(BUFFER, self.writer(File::create(&queue_path)?));
        let roots = self.collect_canonical_roots()?;
        for root in &roots {
            write_record(&mut queue, root, true)?;
        }
        queue.flush()?;
        let mut pending = BufReader::with_capacity(BUFFER, File::open(queue_path)?);
        let mut matches = BufWriter::with_capacity(
            BUFFER,
            self.writer(
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(self.matches_path())?,
            ),
        );
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut boundary = 0;
        let mut match_count = 0;
        let mut scanned = 0;
        let mut last_publish = Instant::now();
        if !self.scope.direct_only {
            for root in &roots {
                if let Some(hit) = self.query.rank(root, true) {
                    boundary += write_match_record(&mut matches, &hit)?;
                    match_count += 1;
                    insert_top(&mut top, hit);
                }
            }
        }
        let mut pending_position = 0;
        loop {
            let record = read_record(&mut pending);
            let partial = record.as_ref().is_ok_and(Option::is_none)
                || record.as_ref().err().is_some_and(|error| {
                    error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::UnexpectedEof)
                });
            let record = if partial && !queue.buffer().is_empty() {
                queue.flush()?;
                pending.seek(SeekFrom::Start(pending_position))?;
                read_record(&mut pending)?
            } else {
                record?
            };
            let Some((directory, _, bytes)) = record else {
                break;
            };
            pending_position += bytes;
            if cancelled() || self.cancelled.load(AtomicOrdering::Acquire) {
                bail!("search cancelled");
            }
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    if roots.contains(&directory) {
                        self.root_issue(&directory, &error);
                    } else {
                        self.io_issue(&directory, &error);
                    }
                    continue;
                }
            };
            for entry in entries {
                if cancelled() || self.cancelled.load(AtomicOrdering::Acquire) {
                    bail!("search cancelled");
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(error) => {
                        self.io_issue(&directory, &error);
                        continue;
                    }
                };
                let path = entry.path();
                if path.starts_with(self.directory.path()) {
                    continue;
                }
                let name = entry.file_name();
                if name.to_str().is_none() {
                    self.issue(format!("Non-Unicode filename: {}", path.display()));
                }
                if !self.scope.options.include_hidden {
                    match anywhere_platform::search_storage::is_hidden(&path, &name) {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(error) => {
                            if let Some(io) = error.downcast_ref::<std::io::Error>() {
                                self.io_issue(&path, io);
                            } else {
                                self.issue(format!("{}: {error}", path.display()));
                            }
                            continue;
                        }
                    }
                }
                let kind = match entry.file_type() {
                    Ok(kind) => kind,
                    Err(error) => {
                        self.io_issue(&path, &error);
                        continue;
                    }
                };
                if self.scope.system_exclusions.excludes(&path, kind.is_dir()) {
                    continue;
                }
                if kind.is_symlink() && !self.scope.authorized_link_target(&path) {
                    continue;
                }
                if self.scope.options.restrict_to_roots
                    && kind.is_dir()
                    && self.query.rank(&path, true).is_some()
                {
                    if let Err(error) = std::fs::read_dir(&path) {
                        self.io_issue(&path, &error);
                        continue;
                    }
                }
                // Classify a link's target when readable, but only real directories
                // enter the queue. Broken links remain visible filename entries.
                let directory = kind.is_dir() || (kind.is_symlink() && path.is_dir());
                if directory
                    && self
                        .scope
                        .options
                        .excluded_directory_names
                        .iter()
                        .any(|excluded| name == excluded.as_str())
                {
                    continue;
                }
                if kind.is_dir() && !self.scope.direct_only {
                    write_record(&mut queue, &path, true)?;
                }
                scanned += 1;
                if (kind.is_file() || directory || kind.is_symlink())
                    && self
                        .scope
                        .exact_path
                        .as_ref()
                        .is_none_or(|exact| exact == &path)
                {
                    if let Some(hit) = self.query.rank(&path, directory) {
                        boundary += write_match_record(&mut matches, &hit)?;
                        match_count += 1;
                        insert_top(&mut top, hit);
                    }
                }
                if last_publish.elapsed() >= Duration::from_millis(100) {
                    matches.flush()?;
                    {
                        let mut p = crate::core::sync::lock_for_revoke(
                            &self.progress,
                            "file_search_progress",
                        );
                        p.boundary = boundary;
                        p.scanned = scanned;
                        p.matches = match_count;
                    }
                    let progress = self.page_progress(boundary, match_count > PAGE_SIZE as u64);
                    publish(ScanPage {
                        hits: top.clone(),
                        progress,
                    })?;
                    last_publish = Instant::now();
                }
            }
        }
        drop(pending);
        drop(queue);
        drop(_queue_cleanup);
        matches.flush()?;
        {
            let mut p = crate::core::sync::lock_for_revoke(&self.progress, "file_search_progress");
            p.boundary = boundary;
            p.scanned = scanned;
            p.matches = match_count;
            p.finished = true;
        }
        let progress = self.page_progress(boundary, match_count > PAGE_SIZE as u64);
        publish(ScanPage {
            hits: top,
            progress,
        })?;
        Ok(())
    }

    pub(super) fn collect_canonical_roots(&self) -> Result<Vec<std::path::PathBuf>> {
        let mut roots = Vec::new();
        for root in self.scope.configured_roots()? {
            match root.canonicalize() {
                Ok(root) if root.is_dir() => {
                    if !roots.contains(&root) {
                        roots.push(root);
                    }
                }
                Ok(_) => self.issue(format!(
                    "Search root is not a directory: {}",
                    root.display()
                )),
                Err(error) => self.root_issue(&root, &error),
            }
        }
        let outer = roots.clone();
        roots.retain(|root| {
            !outer
                .iter()
                .any(|other| other != root && root.starts_with(other))
        });
        Ok(roots)
    }
}
