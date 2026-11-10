//! Disk-backed FIFO traversal and flush-before-publication of complete records.
use super::{
    query::insert_top,
    record::{read_record, write_record},
    ScanPage, ScanStore, BUFFER, PAGE_SIZE,
};
use anyhow::{bail, Result};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
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
        let mut queue = BufWriter::with_capacity(BUFFER, File::create(&queue_path)?);
        let mut roots = Vec::new();
        let configured = self.scope.configured_roots()?;
        for root in configured {
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
        for root in &roots {
            write_record(&mut queue, root, true)?;
        }
        queue.flush()?;
        let mut pending = BufReader::with_capacity(BUFFER, File::open(queue_path)?);
        let mut matches = BufWriter::with_capacity(
            BUFFER,
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(self.matches_path())?,
        );
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut boundary = 0;
        let mut match_count = 0;
        let mut scanned = 0;
        let mut last_publish = Instant::now();
        if !self.scope.direct_only {
            for root in &roots {
                if let Some(hit) = self.query.rank(root, true) {
                    boundary += write_record(&mut matches, root, true)?;
                    match_count += 1;
                    insert_top(&mut top, hit);
                }
            }
        }
        while let Some((directory, _, _)) = read_record(&mut pending)? {
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
                        boundary += write_record(&mut matches, &path, directory)?;
                        match_count += 1;
                        insert_top(&mut top, hit);
                    }
                }
                if last_publish.elapsed() >= Duration::from_millis(100) {
                    matches.flush()?;
                    {
                        let mut p = self.progress.lock().unwrap();
                        p.boundary = boundary;
                        p.scanned = scanned;
                        p.matches = match_count;
                    }
                    let progress = self.page_progress(
                        &top,
                        boundary,
                        0,
                        self.progress().confirmed_matches > PAGE_SIZE as u64,
                    )?;
                    publish(ScanPage {
                        hits: top.clone(),
                        progress,
                    })?;
                    last_publish = Instant::now();
                }
            }
            queue.flush()?;
        }
        matches.flush()?;
        {
            let mut p = self.progress.lock().unwrap();
            p.boundary = boundary;
            p.scanned = scanned;
            p.matches = match_count;
            p.finished = true;
        }
        let progress = self.page_progress(
            &top,
            boundary,
            0,
            self.progress().confirmed_matches > PAGE_SIZE as u64,
        )?;
        publish(ScanPage {
            hits: top,
            progress,
        })?;
        Ok(())
    }
}
