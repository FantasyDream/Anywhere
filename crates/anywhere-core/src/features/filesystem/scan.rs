//! Complete, cancellable filename traversal with bounded RAM and query-local storage.
//! Directory work and all matches are streamed to temporary files, never an index.
use anyhow::{bail, Context, Result};
use anywhere_platform::{FileSearchHit, FileSearchOptions};
use anywhere_protocol::{FileSearchFilter, FileSearchProgress};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        Mutex,
    },
    time::{Duration, Instant},
};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

pub const PAGE_SIZE: usize = 64;
const BUFFER: usize = 64 * 1024;
const MAX_RECORD: usize = 256 * 1024;
static NEXT_QUERY: AtomicU64 = AtomicU64::new(1);

pub fn normalize(text: &str) -> String {
    text.nfc().case_fold().nfc().collect()
}

#[derive(Clone)]
pub struct NameQuery {
    words: Vec<String>,
    joined: String,
    filter: FileSearchFilter,
}
impl NameQuery {
    pub fn new(text: &str, filter: FileSearchFilter) -> Self {
        let joined = normalize(text.trim());
        Self {
            words: joined.split_whitespace().map(str::to_owned).collect(),
            joined,
            filter,
        }
    }
    pub fn rank(&self, path: &Path, is_directory: bool) -> Option<RankedHit> {
        let name = path.file_name()?.to_str()?;
        let normalized = normalize(name);
        if !self.words.iter().all(|w| normalized.contains(w))
            || !self.filter.accepts(name, is_directory)
        {
            return None;
        }
        let score = if normalized == self.joined {
            1000
        } else if normalized.starts_with(&self.joined) {
            900
        } else {
            700
        };
        Some(RankedHit {
            path: path.to_owned(),
            is_directory,
            normalized,
            score,
        })
    }
}

trait FilterName {
    fn accepts(self, name: &str, directory: bool) -> bool;
}
impl FilterName for FileSearchFilter {
    fn accepts(self, name: &str, directory: bool) -> bool {
        super::file_types::accepts(self, name, directory)
    }
}

#[derive(Debug, Clone)]
pub struct RankedHit {
    pub path: PathBuf,
    pub is_directory: bool,
    pub normalized: String,
    pub score: u16,
}
impl RankedHit {
    pub fn compare(&self, other: &Self) -> Ordering {
        other
            .score
            .cmp(&self.score)
            .then_with(|| self.normalized.cmp(&other.normalized))
            .then_with(|| self.path.cmp(&other.path))
    }
    pub fn hit(&self) -> FileSearchHit {
        FileSearchHit {
            path: self.path.clone(),
            is_directory: self.is_directory,
        }
    }
}
fn insert_top(top: &mut Vec<RankedHit>, hit: RankedHit) {
    let at = top
        .binary_search_by(|old| old.compare(&hit))
        .unwrap_or_else(|at| at);
    if at >= PAGE_SIZE {
        return;
    }
    top.insert(at, hit);
    if top.len() > PAGE_SIZE {
        top.pop();
    }
}

fn insert_before(top: &mut Vec<RankedHit>, hit: RankedHit) {
    let at = top
        .binary_search_by(|old| hit.compare(old))
        .unwrap_or_else(|at| at);
    if at >= PAGE_SIZE {
        return;
    }
    top.insert(at, hit);
    if top.len() > PAGE_SIZE {
        top.pop();
    }
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    query: u64,
    boundary: u64,
    score: u16,
    normalized: String,
    path: Vec<u8>,
    page: u32,
    #[serde(default)]
    backwards: bool,
}

pub struct ScanPage {
    pub hits: Vec<RankedHit>,
    pub progress: FileSearchProgress,
}
#[derive(Default)]
pub(super) struct Progress {
    scanned: u64,
    matches: u64,
    pub(super) boundary: u64,
    finished: bool,
    issues: u64,
    inaccessible: u64,
    examples: Vec<String>,
}

pub struct ScanStore {
    // Release the lease before deleting the workspace (important on Windows).
    directory: anywhere_platform::search_storage::SearchWorkspace,
    pub query_id: u64,
    pub(super) query: NameQuery,
    options: FileSearchOptions,
    direct_only: bool,
    exact_path: Option<PathBuf>,
    pub(super) progress: Mutex<Progress>,
    continuous: Mutex<super::result_store::ContinuousResults>,
    pub cancelled: AtomicBool,
}
impl ScanStore {
    pub fn new(text: &str, filter: FileSearchFilter, options: FileSearchOptions) -> Result<Self> {
        let (text, options, direct_only, exact_path) = prepare_query(text, options)?;
        let directory = anywhere_platform::search_storage::SearchWorkspace::new()?;
        File::create(directory.path().join("matches"))?;
        Ok(Self {
            directory,
            query_id: NEXT_QUERY.fetch_add(1, AtomicOrdering::Relaxed),
            query: NameQuery::new(&text, filter),
            options,
            direct_only,
            exact_path,
            progress: Mutex::new(Progress::default()),
            continuous: Mutex::new(Default::default()),
            cancelled: AtomicBool::new(false),
        })
    }
    fn issue(&self, message: impl Into<String>) {
        let mut p = self.progress.lock().expect("scan progress poisoned");
        p.issues += 1;
        if p.examples.len() < 8 {
            p.examples.push(message.into());
        }
    }
    fn io_issue(&self, path: &Path, error: &std::io::Error) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            self.progress
                .lock()
                .expect("scan progress poisoned")
                .inaccessible += 1;
        } else {
            self.issue(format!("{}: {error}", path.display()));
        }
    }
    pub fn fail(&self, error: &anyhow::Error) {
        self.issue(error.to_string());
        self.progress.lock().unwrap().finished = true;
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::Release);
    }
    pub fn is_path_query(&self) -> bool {
        self.direct_only
    }
    fn authorized_link_target(&self, path: &Path) -> bool {
        !self.options.restrict_to_roots
            || path.canonicalize().is_ok_and(|target| {
                self.options.roots.iter().any(|root| {
                    root.canonicalize()
                        .is_ok_and(|root| target.starts_with(root))
                })
            })
    }
    /// Fast candidates obey the same roots, names, hidden and exclusion policy.
    /// They remain provisional until traversal confirms them.
    pub fn verify_index_hits(
        &self,
        hits: Vec<FileSearchHit>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Vec<RankedHit> {
        let roots: Vec<_> = self
            .configured_roots()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|root| root.canonicalize().ok())
            .collect();
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut seen = std::collections::BTreeSet::new();
        for hit in hits.into_iter().take(PAGE_SIZE) {
            if cancelled() {
                break;
            }
            if !seen.insert(hit.path.clone()) {
                continue;
            }
            let Some(ranked) = self.query.rank(&hit.path, hit.is_directory) else {
                continue;
            };
            let Ok(kind) = std::fs::symlink_metadata(&hit.path).map(|m| m.file_type()) else {
                continue;
            };
            if kind.is_symlink() && !self.authorized_link_target(&hit.path) {
                continue;
            }
            // Spotlight can expose metadata from a directory whose contents
            // cannot be read. Such a hit is not an authorized scan result.
            if self.options.restrict_to_roots {
                let directory = if kind.is_dir() {
                    hit.path.as_path()
                } else {
                    let Some(parent) = hit.path.parent() else {
                        continue;
                    };
                    parent
                };
                if std::fs::read_dir(directory).is_err() {
                    continue;
                }
            }
            if kind.is_dir() != hit.is_directory
                || !(kind.is_dir() || kind.is_file() || kind.is_symlink())
            {
                continue;
            }
            let allowed = roots.iter().any(|root| {
                let Ok(relative) = hit.path.strip_prefix(root) else {
                    return false;
                };
                let mut path = root.clone();
                for part in relative.components() {
                    path.push(part.as_os_str());
                    if !self.options.include_hidden
                        && anywhere_platform::search_storage::is_hidden(&path, part.as_os_str())
                            .unwrap_or(true)
                    {
                        return false;
                    }
                    if (path != hit.path || kind.is_dir())
                        && self
                            .options
                            .excluded_directory_names
                            .iter()
                            .any(|name| part.as_os_str() == name.as_str())
                    {
                        return false;
                    }
                    // Descendants through a directory link are outside traversal coverage.
                    if path != hit.path
                        && path
                            .symlink_metadata()
                            .map_or(true, |m| m.file_type().is_symlink())
                    {
                        return false;
                    }
                }
                true
            });
            if allowed {
                insert_top(&mut top, ranked);
            }
        }
        top
    }
    fn configured_roots(&self) -> Result<Vec<PathBuf>> {
        if self.options.roots.is_empty() {
            Ok(vec![std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .context("home directory unavailable")?])
        } else {
            Ok(self.options.roots.clone())
        }
    }
    pub fn progress(&self) -> FileSearchProgress {
        let p = self.progress.lock().unwrap();
        FileSearchProgress {
            query_id: self.query_id,
            roots: if self.options.roots.is_empty() {
                vec!["Home".into()]
            } else {
                self.options
                    .roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect()
            },
            scanned_entries: p.scanned,
            confirmed_matches: p.matches,
            finished: p.finished,
            issue_count: p.issues,
            inaccessible_count: p.inaccessible,
            issue_examples: p.examples.clone(),
            ..Default::default()
        }
    }
    pub fn matches_path(&self) -> PathBuf {
        self.directory.path().join("matches")
    }
    pub fn workspace(&self) -> &Path {
        self.directory.path()
    }
    pub(super) fn page_progress(
        &self,
        top: &[RankedHit],
        boundary: u64,
        page: u32,
        more: bool,
    ) -> Result<FileSearchProgress> {
        let mut progress = self.progress();
        progress.page = page;
        progress.next_cursor = if more {
            let next_page = page.checked_add(1).context("invalid page number")?;
            top.last()
                .map(|hit| {
                    serde_json::to_string(&Cursor {
                        query: self.query_id,
                        boundary,
                        score: hit.score,
                        normalized: hit.normalized.clone(),
                        path: encode_path(&hit.path),
                        page: next_page,
                        backwards: false,
                    })
                })
                .transpose()?
        } else {
            None
        };
        progress.previous_cursor = if page > 0 {
            top.first()
                .map(|hit| {
                    serde_json::to_string(&Cursor {
                        query: self.query_id,
                        boundary,
                        score: hit.score,
                        normalized: hit.normalized.clone(),
                        path: encode_path(&hit.path),
                        page: page - 1,
                        backwards: true,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(progress)
    }
    pub fn page(
        &self,
        cursor: Option<&str>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<ScanPage> {
        let (boundary, after, page, backwards) = if let Some(cursor) = cursor {
            if cursor.len() > MAX_RECORD * 4 {
                bail!("invalid page cursor");
            }
            let cursor: Cursor = serde_json::from_str(cursor).context("invalid page cursor")?;
            let progress = self.progress.lock().unwrap();
            if cursor.query != self.query_id
                || cursor.boundary > progress.boundary
                || u64::from(cursor.page) > progress.matches / PAGE_SIZE as u64
            {
                bail!("stale page cursor");
            }
            let hit = RankedHit {
                path: decode_path(cursor.path)?,
                is_directory: false,
                normalized: cursor.normalized,
                score: cursor.score,
            };
            (cursor.boundary, Some(hit), cursor.page, cursor.backwards)
        } else {
            (self.progress.lock().unwrap().boundary, None, 0, false)
        };
        let mut reader =
            BufReader::with_capacity(BUFFER, File::open(self.matches_path())?.take(boundary));
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut eligible = 0;
        while let Some((path, directory, _bytes)) = read_record(&mut reader)? {
            if self.cancelled.load(AtomicOrdering::Acquire) || cancelled() {
                bail!("search cancelled");
            }
            let hit = self
                .query
                .rank(&path, directory)
                .context("invalid match record")?;
            if after.as_ref().is_some_and(|after| {
                hit.compare(after)
                    != if backwards {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
            }) {
                continue;
            }
            eligible += 1;
            if backwards {
                insert_before(&mut top, hit);
            } else {
                insert_top(&mut top, hit);
            }
        }
        if backwards {
            top.reverse();
        }
        let mut progress =
            self.page_progress(&top, boundary, page, backwards || eligible > PAGE_SIZE)?;
        progress.needs_refresh = boundary != self.progress.lock().unwrap().boundary;
        Ok(ScanPage {
            hits: top,
            progress,
        })
    }

    /// Read another bounded batch for the continuous list. Re-read the latest
    /// committed boundary so late, higher-ranked matches are not skipped by an
    /// earlier sort-key cursor. Only targets already displayed are excluded.
    pub fn more(
        &self,
        displayed: &std::collections::HashSet<PathBuf>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<ScanPage> {
        let mut continuous = self.continuous.lock().unwrap();
        let result = continuous.more(self, displayed, &|| {
            self.cancelled.load(AtomicOrdering::Acquire) || cancelled()
        });
        if result.is_err() {
            continuous.invalidate();
        }
        result
    }

    pub fn run(
        &self,
        cancelled: &(dyn Fn() -> bool + Sync),
        mut publish: impl FnMut(ScanPage) -> Result<()>,
    ) -> Result<()> {
        let queue_path = self.directory.path().join("pending-dirs");
        let mut queue = BufWriter::with_capacity(BUFFER, File::create(&queue_path)?);
        let mut roots = Vec::new();
        let configured = self.configured_roots()?;
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
                Err(error) => self.io_issue(&root, &error),
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
        if !self.direct_only {
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
                    self.io_issue(&directory, &error);
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
                if !self.options.include_hidden {
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
                if kind.is_symlink() && !self.authorized_link_target(&path) {
                    continue;
                }
                if self.options.restrict_to_roots
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
                        .options
                        .excluded_directory_names
                        .iter()
                        .any(|excluded| name == excluded.as_str())
                {
                    continue;
                }
                if kind.is_dir() && !self.direct_only {
                    write_record(&mut queue, &path, true)?;
                }
                scanned += 1;
                if (kind.is_file() || directory || kind.is_symlink())
                    && self.exact_path.as_ref().is_none_or(|exact| exact == &path)
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

/// Explicit paths browse one directory instead of repeatedly scanning an entire tree.
fn prepare_query(
    text: &str,
    mut options: FileSearchOptions,
) -> Result<(String, FileSearchOptions, bool, Option<PathBuf>)> {
    let raw = text.trim();
    let path = if let Some(tail) = raw.strip_prefix("~/") {
        PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .context("home directory unavailable")?,
        )
        .join(tail)
    } else {
        PathBuf::from(raw)
    };
    if path.components().count() <= 1 && !path.is_absolute() {
        return Ok((raw.into(), options, false, None));
    }
    let path = if path.is_absolute() {
        path
    } else {
        let base = options
            .roots
            .first()
            .cloned()
            .or_else(|| {
                std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
            })
            .context("search directory unavailable")?;
        base.join(path)
    };
    if options.restrict_to_roots {
        let lexical = anywhere_platform::search_storage::lexical_scope_path(&path);
        if !options.roots.iter().any(|root| {
            lexical.starts_with(anywhere_platform::search_storage::lexical_scope_path(root))
        }) {
            bail!("This path is outside the authorized search directories. Update Search settings to authorize it.");
        }
        // A lexical child can be a link into an unapproved directory. Check the
        // resolved target as well, before enumerating it.
        let target = path.canonicalize().or_else(|_| {
            path.parent()
                .context("path has no parent directory")?
                .canonicalize()
                .map_err(anyhow::Error::from)
        })?;
        if !options.roots.iter().any(|root| {
            root.canonicalize()
                .is_ok_and(|root| target.starts_with(root))
        }) {
            bail!("This path is outside the authorized search directories. Update Search settings to authorize it.");
        }
    }
    let (root, name, exact) = if path.is_dir() {
        (path.clone(), String::new(), None)
    } else {
        (
            path.parent()
                .context("path has no parent directory")?
                .to_owned(),
            path.file_name()
                .and_then(|name| name.to_str())
                .context("invalid filename")?
                .to_owned(),
            if path.is_file() {
                Some(path.clone())
            } else {
                None
            },
        )
    };
    let root = root.canonicalize().unwrap_or(root);
    let exact = exact.map(|path| root.join(path.file_name().expect("validated filename")));
    options.roots = vec![root];
    Ok((name, options, true, exact))
}

use anywhere_platform::search_storage::{decode_path, encode_path};

pub(super) fn write_record(writer: &mut impl Write, path: &Path, directory: bool) -> Result<u64> {
    let bytes = encode_path(path);
    if bytes.len() > MAX_RECORD {
        bail!("path exceeds record size");
    }
    writer.write_all(&(bytes.len() as u32 + 1).to_le_bytes())?;
    writer.write_all(&[u8::from(directory)])?;
    writer.write_all(&bytes)?;
    Ok(bytes.len() as u64 + 5)
}
pub(super) fn read_record(reader: &mut impl Read) -> Result<Option<(PathBuf, bool, u64)>> {
    let mut length = [0; 4];
    if reader.read(&mut length[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut length[1..])?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_RECORD + 1 {
        bail!("invalid path record length");
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    let directory = bytes.remove(0) != 0;
    Ok(Some((decode_path(bytes)?, directory, length as u64 + 4)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn options(root: &Path) -> FileSearchOptions {
        FileSearchOptions {
            roots: vec![root.canonicalize().unwrap()],
            include_hidden: true,
            ..Default::default()
        }
    }
    fn finish(store: &ScanStore) -> ScanPage {
        let mut final_page = None;
        store
            .run(&|| false, |page| {
                final_page = Some(page);
                Ok(())
            })
            .unwrap();
        final_page.unwrap()
    }
    fn all_pages(store: &ScanStore, first: ScanPage) -> BTreeSet<PathBuf> {
        let mut page = first;
        let mut all = BTreeSet::new();
        loop {
            assert!(page.hits.len() <= PAGE_SIZE);
            for hit in page.hits {
                assert!(all.insert(hit.path), "duplicate page result");
            }
            match page.progress.next_cursor {
                Some(cursor) => page = store.page(Some(&cursor), &|| false).unwrap(),
                None => break,
            }
        }
        all
    }

    #[test]
    fn failed_continuous_read_can_retry_without_omitting_unread_records() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..193 {
            std::fs::write(fixture.path().join(format!("report-{i:03}.txt")), "").unwrap();
        }
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        finish(&store);
        let mut displayed: std::collections::HashSet<_> = store
            .more(&std::collections::HashSet::new(), &|| false)
            .unwrap()
            .hits
            .into_iter()
            .map(|h| h.path)
            .collect();
        for entry in std::fs::read_dir(store.workspace()).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with("sorted-") {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        assert!(store.more(&displayed, &|| false).is_err());
        loop {
            let next = store.more(&displayed, &|| false).unwrap();
            if next.hits.is_empty() {
                break;
            }
            for hit in next.hits {
                assert!(displayed.insert(hit.path));
            }
        }
        assert_eq!(displayed.len(), 193);
    }

    #[test]
    fn continuous_external_sort_reaches_every_match_across_merge_levels() {
        let fixture = tempfile::tempdir().unwrap();
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        let mut writer = BufWriter::new(File::create(store.matches_path()).unwrap());
        let mut boundary = 0;
        for i in (0..17001).rev() {
            boundary += write_record(
                &mut writer,
                &fixture.path().join(format!("report-{i:05}.txt")),
                false,
            )
            .unwrap();
        }
        writer.flush().unwrap();
        {
            let mut progress = store.progress.lock().unwrap();
            progress.boundary = boundary;
            progress.matches = 17001;
            progress.finished = true;
        }
        let mut displayed = std::collections::HashSet::new();
        let mut names = Vec::new();
        loop {
            let batch = store.more(&displayed, &|| false).unwrap();
            if batch.hits.is_empty() {
                break;
            }
            for hit in batch.hits {
                names.push(hit.path.file_name().unwrap().to_string_lossy().into_owned());
                assert!(displayed.insert(hit.path));
            }
        }
        assert_eq!(displayed.len(), 17001);
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(
            std::fs::read_dir(store.workspace())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("sorted-"))
                .count(),
            1
        );
    }

    #[test]
    fn continuous_batches_include_late_higher_ranked_matches_without_duplicates() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..150 {
            std::fs::write(fixture.path().join(format!("report-{i:03}.pdf")), "").unwrap();
        }
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        let first = finish(&store);
        let mut displayed: std::collections::HashSet<_> =
            first.hits.into_iter().map(|h| h.path).collect();
        // Simulate a later committed scan batch whose exact match sorts before
        // every displayed row. A forward-only old cursor would miss this hit.
        let late = fixture.path().canonicalize().unwrap().join("report");
        std::fs::write(&late, "").unwrap();
        let mut file = OpenOptions::new()
            .append(true)
            .open(store.matches_path())
            .unwrap();
        let bytes = write_record(&mut file, &late, false).unwrap();
        file.flush().unwrap();
        {
            let mut progress = store.progress.lock().unwrap();
            progress.boundary += bytes;
            progress.matches += 1;
        }
        let second = store.more(&displayed, &|| false).unwrap();
        assert_eq!(second.hits[0].path, late);
        for hit in second.hits {
            assert!(displayed.insert(hit.path));
        }
        loop {
            let batch = store.more(&displayed, &|| false).unwrap();
            assert!(batch.hits.len() <= PAGE_SIZE);
            for hit in batch.hits {
                assert!(displayed.insert(hit.path));
            }
            if batch.progress.next_cursor.is_none() {
                break;
            }
        }
        assert_eq!(displayed.len(), 151);
        assert!(store.more(&displayed, &|| false).unwrap().hits.is_empty());
        assert!(store.more(&displayed, &|| true).is_err());
    }

    #[test]
    fn all_matches_are_reachable_past_two_pages_with_overlapping_roots_and_folders() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let child = root.join("report-folder");
        std::fs::create_dir(&child).unwrap();
        let mut expected = BTreeSet::from([child.clone()]);
        for i in 0..193 {
            let path = root.join(format!("report-{i:04}.txt"));
            std::fs::write(&path, "").unwrap();
            expected.insert(path);
        }
        let nested = child.join("report-nested.pdf");
        std::fs::write(&nested, "").unwrap();
        expected.insert(nested);
        let mut options = options(&root);
        options.roots.extend([child.clone(), root]);
        let store = ScanStore::new("report", FileSearchFilter::All, options).unwrap();
        let page = finish(&store);
        assert!(page.progress.finished);
        assert_eq!(page.progress.issue_count, 0);
        assert_eq!(page.progress.confirmed_matches, expected.len() as u64);
        assert_eq!(all_pages(&store, page), expected);
        let workspace = store.workspace().to_owned();
        drop(store);
        assert!(!workspace.exists());
    }

    #[test]
    fn filters_run_before_page_limits_and_hidden_ignored_noise_directories_are_searched() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        for i in 0..130 {
            std::fs::write(root.join(format!("report-{i:04}.txt")), "").unwrap();
        }
        for name in [
            ".hidden",
            "node_modules",
            "Library",
            "target",
            "build",
            "Package.app",
        ] {
            std::fs::create_dir(root.join(name)).unwrap();
        }
        std::fs::write(root.join(".gitignore"), "*.pdf\n").unwrap();
        let mut expected = BTreeSet::new();
        for name in [
            ".hidden",
            "node_modules",
            "Library",
            "target",
            "build",
            "Package.app",
        ] {
            let path = root.join(name).join("report-hidden.PDF");
            std::fs::write(&path, "").unwrap();
            expected.insert(path);
        }
        let store = ScanStore::new("report", FileSearchFilter::Pdf, options(&root)).unwrap();
        assert_eq!(all_pages(&store, finish(&store)), expected);
        let mut visible = options(&root);
        visible.include_hidden = false;
        visible.excluded_directory_names = vec!["node_modules".into()];
        let store = ScanStore::new("report", FileSearchFilter::Pdf, visible).unwrap();
        let all = all_pages(&store, finish(&store));
        assert_eq!(all.len(), 4);
        assert!(!all.iter().any(|p| p
            .components()
            .any(|c| c.as_os_str() == ".hidden" || c.as_os_str() == "node_modules")));
    }

    #[test]
    fn reverse_paging_is_stable_without_retaining_a_cursor_history() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..150 {
            std::fs::write(fixture.path().join(format!("report-{i:03}")), "").unwrap();
        }
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        let first = finish(&store);
        let first_paths: Vec<_> = first.hits.iter().map(|h| h.path.clone()).collect();
        let second = store
            .page(first.progress.next_cursor.as_deref(), &|| false)
            .unwrap();
        let third = store
            .page(second.progress.next_cursor.as_deref(), &|| false)
            .unwrap();
        let second_again = store
            .page(third.progress.previous_cursor.as_deref(), &|| false)
            .unwrap();
        assert_eq!(
            second
                .hits
                .iter()
                .map(|h| h.path.clone())
                .collect::<Vec<_>>(),
            second_again
                .hits
                .iter()
                .map(|h| h.path.clone())
                .collect::<Vec<_>>()
        );
        let first_again = store
            .page(second_again.progress.previous_cursor.as_deref(), &|| false)
            .unwrap();
        assert_eq!(
            first_paths,
            first_again
                .hits
                .iter()
                .map(|h| h.path.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(first_again.progress.page, 0);
    }

    #[test]
    fn permission_denial_is_skipped_without_hiding_accessible_matches() {
        let fixture = tempfile::tempdir().unwrap();
        std::fs::write(fixture.path().join("report.txt"), "").unwrap();
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        store.io_issue(
            fixture.path(),
            &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        let page = finish(&store);
        assert!(page.progress.finished);
        assert_eq!(page.progress.inaccessible_count, 1);
        assert_eq!(page.progress.issue_count, 0);
        assert!(page.progress.issue_examples.is_empty());
        assert_eq!(page.hits.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_directory_is_skipped_and_other_files_remain_searchable() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let blocked = fixture.path().join("report-protected");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("report-secret.txt"), "").unwrap();
        std::fs::write(fixture.path().join("report-readable.txt"), "").unwrap();
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::read_dir(&blocked).is_err();
        let mut scoped = options(fixture.path());
        scoped.restrict_to_roots = true;
        let store = ScanStore::new("report", FileSearchFilter::All, scoped).unwrap();
        if denied {
            assert!(store
                .verify_index_hits(
                    vec![FileSearchHit {
                        path: blocked.clone(),
                        is_directory: true
                    }],
                    &|| false
                )
                .is_empty());
        }
        let page = finish(&store);
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700)).unwrap();
        if denied {
            assert_eq!(page.progress.inaccessible_count, 1);
            assert_eq!(page.hits.len(), 1);
        }
        assert!(page.progress.finished);
        assert_eq!(page.progress.issue_count, 0);
    }

    #[test]
    fn authorized_scope_rejects_explicit_paths_and_links_that_escape_it() {
        let fixture = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let mut scoped = options(fixture.path());
        scoped.restrict_to_roots = true;
        std::fs::write(outside.path().join("report.txt"), "").unwrap();
        assert!(ScanStore::new(
            outside.path().to_str().unwrap(),
            FileSearchFilter::All,
            scoped.clone()
        )
        .is_err());
        let escape = fixture
            .path()
            .join("..")
            .join(outside.path().file_name().unwrap());
        assert!(ScanStore::new(
            escape.to_str().unwrap(),
            FileSearchFilter::All,
            scoped.clone()
        )
        .is_err());
        #[cfg(unix)]
        {
            let link = fixture.path().join("link");
            std::os::unix::fs::symlink(outside.path(), &link).unwrap();
            assert!(ScanStore::new(
                link.to_str().unwrap(),
                FileSearchFilter::All,
                scoped.clone()
            )
            .is_err());
        }
        std::fs::write(fixture.path().join("report.txt"), "").unwrap();
        let store = ScanStore::new(
            fixture.path().to_str().unwrap(),
            FileSearchFilter::All,
            scoped,
        )
        .unwrap();
        assert_eq!(finish(&store).hits.len(), 1);
    }

    #[test]
    fn missing_roots_report_partial_but_do_not_hide_other_roots() {
        let fixture = tempfile::tempdir().unwrap();
        let mut options = options(fixture.path());
        std::fs::write(fixture.path().join("report.txt"), "").unwrap();
        options.roots.insert(0, fixture.path().join("missing"));
        let store = ScanStore::new("report", FileSearchFilter::All, options).unwrap();
        let page = finish(&store);
        assert!(page.progress.finished);
        assert_eq!(page.progress.issue_count, 1);
        assert_eq!(page.hits.len(), 1);
    }

    #[test]
    fn literal_short_unicode_names_share_one_matching_contract() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        for name in [
            "报告[季度].md",
            "Café.txt",
            "Straße.md",
            "report\nline.txt",
            "abbax.txt",
        ] {
            std::fs::write(root.join(name), "").unwrap();
        }
        for (query, expected) in [
            ("报", "报告[季度].md"),
            ("[季度]", "报告[季度].md"),
            ("cafe\u{301}", "Café.txt"),
            ("STRASSE", "Straße.md"),
            ("line", "report\nline.txt"),
        ] {
            let store = ScanStore::new(query, FileSearchFilter::All, options(&root)).unwrap();
            let page = finish(&store);
            assert_eq!(
                page.hits
                    .iter()
                    .map(|h| h.path.file_name().unwrap().to_str().unwrap())
                    .collect::<Vec<_>>(),
                [expected]
            );
        }
        let store = ScanStore::new("aba", FileSearchFilter::All, options(&root)).unwrap();
        assert!(finish(&store).hits.is_empty());
    }

    #[test]
    fn cancellation_and_foreign_cursors_do_not_return_complete_results() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..65 {
            std::fs::write(fixture.path().join(format!("report-{i}")), "").unwrap();
        }
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        assert!(store
            .run(&|| true, |_| panic!("cancelled query published"))
            .is_err());
        assert!(!store.progress().finished);
        let first = finish(&store);
        let cursor = first.progress.next_cursor.unwrap();
        let another =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        finish(&another);
        assert!(another.page(Some(&cursor), &|| false).is_err());
        assert!(store.page(Some(&cursor), &|| true).is_err());
        let mut forged: serde_json::Value = serde_json::from_str(&cursor).unwrap();
        forged["page"] = u32::MAX.into();
        assert!(store.page(Some(&forged.to_string()), &|| false).is_err());
    }

    #[test]
    fn explicit_paths_browse_direct_children_and_exact_files_without_sibling_leaks() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        std::fs::create_dir(root.join("nested")).unwrap();
        std::fs::write(root.join("report.txt"), "").unwrap();
        std::fs::write(root.join("report.txt.bak"), "").unwrap();
        std::fs::write(root.join("nested/report-deep.txt"), "").unwrap();
        let store =
            ScanStore::new(root.to_str().unwrap(), FileSearchFilter::All, options(root)).unwrap();
        let page = finish(&store);
        assert_eq!(page.hits.len(), 3);
        assert!(page
            .hits
            .iter()
            .all(|h| h.path.parent() == Some(root.canonicalize().unwrap().as_path())));
        for (path, count) in [(root.join("report.txt"), 1), (root.join("report"), 2)] {
            let store =
                ScanStore::new(path.to_str().unwrap(), FileSearchFilter::All, options(root))
                    .unwrap();
            let page = finish(&store);
            assert_eq!(page.hits.len(), count);
            assert!(page.progress.finished && page.progress.issue_count == 0);
        }
    }

    #[test]
    fn provisional_hits_obey_roots_type_hidden_exclusions_existence_and_name() {
        let fixture = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        for dir in [".hidden", "excluded"] {
            std::fs::create_dir(root.join(dir)).unwrap();
        }
        let paths = [
            root.join("report.pdf"),
            root.join(".hidden/report.pdf"),
            root.join("excluded/report.pdf"),
            root.join("report.txt"),
            outside.path().join("report.pdf"),
        ];
        for path in &paths {
            std::fs::write(path, "").unwrap();
        }
        let mut policy = options(&root);
        policy.include_hidden = false;
        policy.excluded_directory_names = vec!["excluded".into()];
        let store = ScanStore::new("report", FileSearchFilter::Pdf, policy).unwrap();
        let mut hits: Vec<_> = paths
            .into_iter()
            .map(|path| FileSearchHit {
                path,
                is_directory: false,
            })
            .collect();
        hits.extend([
            FileSearchHit {
                path: root.join("missing-report.pdf"),
                is_directory: false,
            },
            FileSearchHit {
                path: root.join("report.pdf"),
                is_directory: false,
            },
        ]);
        let verified = store.verify_index_hits(hits, &|| false);
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].path, root.join("report.pdf"));
        assert_eq!(finish(&store).hits.len(), 1);
    }

    #[test]
    fn batches_commit_complete_records_and_old_page_boundaries_require_refresh() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..170 {
            std::fs::write(fixture.path().join(format!("report-{i:03}")), "").unwrap();
        }
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let mut partial_cursor = None;
        store
            .run(
                &|| {
                    if calls.fetch_add(1, AtomicOrdering::Relaxed) == 100 {
                        std::thread::sleep(Duration::from_millis(105));
                    }
                    false
                },
                |page| {
                    let committed = store.page(None, &|| false)?;
                    assert_eq!(
                        committed.progress.confirmed_matches,
                        page.progress.confirmed_matches
                    );
                    if !page.progress.finished {
                        assert!(
                            page.progress.confirmed_matches > 0
                                && page.progress.confirmed_matches < 170
                        );
                        partial_cursor = page.progress.next_cursor;
                    }
                    Ok(())
                },
            )
            .unwrap();
        let cursor = partial_cursor.expect("intermediate batch with more than one page");
        let stale_boundary = store.page(Some(&cursor), &|| false).unwrap();
        assert!(stale_boundary.progress.needs_refresh);
        assert!(store.progress().finished);
        assert_eq!(
            all_pages(&store, store.page(None, &|| false).unwrap()).len(),
            170
        );
    }

    #[test]
    fn broken_storage_is_an_error_and_cannot_be_reported_complete() {
        let fixture = tempfile::tempdir().unwrap();
        let store =
            ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
        std::fs::remove_file(store.matches_path()).unwrap();
        let error = store
            .run(&|| false, |_| panic!("failed storage published success"))
            .unwrap_err();
        store.fail(&error);
        assert!(store.progress().finished && store.progress().issue_count > 0);
        assert!(store.page(None, &|| false).is_err());
    }

    #[test]
    #[ignore = "large filesystem fixture; run explicitly for coverage regression"]
    fn complete_scan_has_no_fifty_thousand_entry_cutoff() {
        let fixture = tempfile::tempdir().unwrap();
        for i in 0..50_100 {
            std::fs::write(fixture.path().join(format!("entry-{i:05}")), "").unwrap();
        }
        std::fs::write(fixture.path().join("needle.pdf"), "").unwrap();
        let store =
            ScanStore::new("needle", FileSearchFilter::Pdf, options(fixture.path())).unwrap();
        let page = finish(&store);
        assert!(page.progress.finished);
        assert_eq!(page.progress.issue_count, 0);
        assert_eq!(page.progress.scanned_entries, 50_101);
        assert_eq!(page.hits.len(), 1);
    }
}
