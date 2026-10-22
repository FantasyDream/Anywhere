//! Query-local external sorting. A loaded batch consumes a cursor instead of
//! re-ranking the whole match file. New committed records merge into the unread tail.
use super::scan::{
    read_record, write_record, NameQuery, RankedHit, ScanPage, ScanStore, PAGE_SIZE,
};
use anyhow::{bail, Context, Result};
use std::{
    collections::{BinaryHeap, HashSet},
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
const FAN_IN: usize = 16;
const SORT_BYTES: usize = 1024 * 1024;
const SORT_ITEMS: usize = 512;

#[derive(Default)]
pub(super) struct ContinuousResults {
    boundary: u64,
    file: Option<PathBuf>,
    position: u64,
    serial: u64,
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
fn next(reader: &mut impl Read, query: &NameQuery) -> Result<Option<RankedHit>> {
    read_record(reader)?
        .map(|(path, directory, _)| query.rank(&path, directory).context("invalid match record"))
        .transpose()
}
fn check(cancelled: &(dyn Fn() -> bool + Sync)) -> Result<()> {
    if cancelled() {
        bail!("search cancelled")
    }
    Ok(())
}
impl ContinuousResults {
    pub fn invalidate(&mut self) {
        self.boundary = 0;
        self.position = 0;
        self.file = None;
        // Keep serial monotonic so retry cannot overwrite an orphaned sort run.
    }

    fn path(&mut self, directory: &Path) -> PathBuf {
        self.serial += 1;
        directory.join(format!("sorted-{}", self.serial))
    }
    fn merge(
        &mut self,
        sources: &[(PathBuf, u64)],
        store: &ScanStore,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<PathBuf> {
        let output = self.path(store.workspace());
        let mut writer = BufWriter::new(File::create(&output)?);
        let mut readers = Vec::new();
        let mut heads = BinaryHeap::new();
        for (source, (path, offset)) in sources.iter().enumerate() {
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(*offset))?;
            let mut reader = BufReader::new(file);
            if let Some(hit) = next(&mut reader, &store.query)? {
                heads.push(Head { hit, source });
            }
            readers.push(reader);
        }
        while let Some(Head { hit, source }) = heads.pop() {
            check(cancelled)?;
            write_record(&mut writer, &hit.path, hit.is_directory)?;
            if let Some(hit) = next(&mut readers[source], &store.query)? {
                heads.push(Head { hit, source });
            }
        }
        writer.flush()?;
        drop(readers);
        for (path, _) in sources {
            std::fs::remove_file(path)?;
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
                let Some(hit) = next(&mut reader, &store.query)? else {
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
            let mut writer = BufWriter::new(File::create(&path)?);
            for hit in hits {
                write_record(&mut writer, &hit.path, hit.is_directory)?;
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
        let boundary = store.progress.lock().unwrap().boundary;
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
            let Some((path, directory, bytes)) = read_record(&mut reader)? else {
                break;
            };
            self.position += bytes;
            if !displayed.contains(&path) {
                hits.push(
                    store
                        .query
                        .rank(&path, directory)
                        .context("invalid match record")?,
                );
            }
        }
        // Peek without consuming; duplicates from a provisional batch are skipped
        // during the next read and can only cause a harmless empty final batch.
        let more = read_record(&mut reader)?.is_some();
        let mut progress = store.page_progress(&hits, boundary, 0, more)?;
        progress.needs_refresh = boundary != store.progress.lock().unwrap().boundary;
        Ok(ScanPage { hits, progress })
    }
}
