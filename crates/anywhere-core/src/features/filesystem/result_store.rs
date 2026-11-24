//! Query-local external sorting. A loaded batch consumes a cursor instead of
//! re-ranking the whole match file. New committed records merge into the unread tail.
use super::scan::{
    read_match_record, write_match_record, RankedHit, ScanPage, ScanStore, PAGE_SIZE,
};
use anyhow::{bail, Result};
use std::{
    collections::{BinaryHeap, HashSet},
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
const FAN_IN: usize = 16;
#[cfg(not(test))]
const SORT_BYTES: usize = 4 * 1024 * 1024;
#[cfg(test)]
const SORT_BYTES: usize = 1024 * 1024;
#[cfg(not(test))]
const SORT_ITEMS: usize = 4096;
#[cfg(test)]
const SORT_ITEMS: usize = 512;

#[derive(Default)]
pub(super) struct ContinuousResults {
    boundary: u64,
    file: Option<PathBuf>,
    position: u64,
    serial: u64,
    allocated: HashSet<PathBuf>,
}
struct Head {
    hit: RankedHit,
    source: usize,
}
impl PartialEq for Head {
    fn eq(&self, other: &Self) -> bool {
        self.hit.compare(&other.hit).is_eq() && self.source == other.source
    }
}
impl Eq for Head {}
impl PartialOrd for Head {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Head {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hit
            .compare(&other.hit)
            .reverse()
            .then(self.source.cmp(&other.source))
    }
}
fn next(reader: &mut impl Read) -> Result<Option<RankedHit>> {
    Ok(read_match_record(reader)?.map(|(hit, _)| hit))
}
fn check(cancelled: &(dyn Fn() -> bool + Sync)) -> Result<()> {
    if cancelled() {
        bail!("search cancelled")
    }
    Ok(())
}
impl ContinuousResults {
    pub fn invalidate(&mut self, store: &ScanStore) {
        self.boundary = 0;
        self.position = 0;
        self.file = None;
        for path in self.allocated.drain() {
            let _ = store.remove_run(&path);
        }
    }

    fn path(&mut self, directory: &Path) -> PathBuf {
        self.serial += 1;
        let path = directory.join(format!("sorted-{}", self.serial));
        self.allocated.insert(path.clone());
        path
    }
    fn merge(
        &mut self,
        sources: &[(PathBuf, u64)],
        store: &ScanStore,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf> {
        let output = self.path(store.workspace());
        let mut writer = BufWriter::new(store.writer(File::create(&output)?));
        let mut readers = Vec::new();
        let mut heads = BinaryHeap::new();
        for (source, (path, offset)) in sources.iter().enumerate() {
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(*offset))?;
            let mut reader = BufReader::new(file);
            if let Some(hit) = next(&mut reader)? {
                heads.push(Head { hit, source });
            }
            readers.push(reader);
        }
        while let Some(Head { hit, source }) = heads.pop() {
            check(cancelled)?;
            write_match_record(&mut writer, &hit)?;
            if let Some(hit) = next(&mut readers[source])? {
                heads.push(Head { hit, source });
            }
        }
        writer.flush()?;
        drop(readers);
        for (path, _) in sources {
            store.remove_run(path)?;
            self.allocated.remove(path);
        }
        Ok(output)
    }
    fn sorted_delta(
        &mut self,
        store: &ScanStore,
        boundary: u64,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf> {
        let mut input = File::open(store.matches_path())?;
        input.seek(SeekFrom::Start(self.boundary))?;
        let mut reader = BufReader::new(input.take(boundary - self.boundary));
        let mut levels: Vec<Vec<PathBuf>> = vec![Vec::new()];
        loop {
            let mut hits = Vec::new();
            let mut bytes = 0;
            while hits.len() < SORT_ITEMS && bytes < SORT_BYTES {
                check(cancelled)?;
                let Some(hit) = next(&mut reader)? else {
                    break;
                };
                bytes += hit.path.as_os_str().len() + hit.normalized.len();
                hits.push(hit);
            }
            if hits.is_empty() {
                break;
            }
            hits.sort_by(RankedHit::compare);
            let path = self.path(store.workspace());
            let mut writer = BufWriter::new(store.writer(File::create(&path)?));
            for hit in hits {
                write_match_record(&mut writer, &hit)?;
            }
            writer.flush()?;
            levels[0].push(path);
            let mut level = 0;
            while levels[level].len() == FAN_IN {
                let sources: Vec<_> = levels[level].drain(..).map(|p| (p, 0)).collect();
                let merged = self.merge(&sources, store, cancelled)?;
                level += 1;
                if level == levels.len() {
                    levels.push(Vec::new());
                }
                levels[level].push(merged);
            }
        }
        let mut paths: Vec<_> = levels.into_iter().flatten().collect();
        if paths.is_empty() {
            let empty = self.path(store.workspace());
            File::create(&empty)?;
            return Ok(empty);
        }
        while paths.len() > 1 {
            let mut reduced = Vec::new();
            for group in paths.chunks(FAN_IN) {
                if group.len() == 1 {
                    reduced.push(group[0].clone());
                } else {
                    reduced.push(self.merge(
                        &group.iter().cloned().map(|p| (p, 0)).collect::<Vec<_>>(),
                        store,
                        cancelled,
                    )?);
                }
            }
            paths = reduced;
        }
        Ok(paths.remove(0))
    }
    pub fn more(
        &mut self,
        store: &ScanStore,
        displayed: &HashSet<PathBuf>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<ScanPage> {
        let boundary = store.committed_boundary();
        if self.file.is_none() || boundary != self.boundary {
            let delta = self.sorted_delta(store, boundary, cancelled)?;
            self.file = Some(if let Some(previous) = self.file.take() {
                self.merge(&[(previous, self.position), (delta, 0)], store, cancelled)?
            } else {
                delta
            });
            self.position = 0;
            self.boundary = boundary;
        }
        let mut file = File::open(self.file.as_ref().unwrap())?;
        file.seek(SeekFrom::Start(self.position))?;
        let mut reader = BufReader::new(file);
        let mut hits = Vec::with_capacity(PAGE_SIZE);
        while hits.len() < PAGE_SIZE {
            check(cancelled)?;
            let Some((hit, bytes)) = read_match_record(&mut reader)? else {
                break;
            };
            self.position += bytes;
            if !displayed.contains(&hit.path) {
                hits.push(hit);
            }
        }
        // Consume already displayed records, but leave the next unseen hit
        // for the next page. A duplicate-only tail is already exhausted.
        let more = loop {
            check(cancelled)?;
            match read_match_record(&mut reader)? {
                Some((hit, _)) if !displayed.contains(&hit.path) => break true,
                Some((_, bytes)) => self.position += bytes,
                None => break false,
            }
        };
        let progress = store.page_progress(boundary, more);
        Ok(ScanPage { hits, progress })
    }
}
