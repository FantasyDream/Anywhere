//! Bounded, cancellable filename enumeration; never reads file contents.
use crate::{
    process::{OwnedProcess, ProcessRequest},
    FileIndexResults, FileSearchEngine, FileSearchHit, FileSearchOptions,
};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_OUTPUT: usize = 8 * 1024 * 1024;
const MAX_PATH: usize = 16 * 1024;
const MAX_ENTRIES: usize = 50_000;
pub(crate) const NOISE_DIRECTORIES: &[&str] = &["Library", "node_modules", "target", "build"];

/// Platform-neutral request passed to every [`FileSearchBackend`] implementation.
pub struct FileSearchQuery<'a> {
    pub text: &'a str,
    pub roots: &'a [PathBuf],
    pub limit: usize,
    pub options: &'a FileSearchOptions,
    pub timeout: Duration,
}

/// Pluggable file search backend (Spotlight, Everything, Windows Search, Ripgrep, etc.).
pub trait FileSearchBackend: Send + Sync {
    fn engine(&self) -> FileSearchEngine;
    fn is_available(&self) -> bool;
    fn search(
        &self,
        query: &FileSearchQuery<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults>;
}

/// Cross-platform Ripgrep (`rg --files`) backend.
pub struct RipgrepBackend;

impl FileSearchBackend for RipgrepBackend {
    fn engine(&self) -> FileSearchEngine {
        FileSearchEngine::Ripgrep
    }

    fn is_available(&self) -> bool {
        crate::process::find_executable("rg").is_some()
    }

    fn search(
        &self,
        query: &FileSearchQuery<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        let mut options = query.options.clone();
        if !query.roots.is_empty() {
            options.roots = query.roots.to_vec();
        }
        let hits = ripgrep(query.text, query.limit, &options, cancelled, query.timeout)?;
        Ok(FileIndexResults {
            truncated: hits.len() >= query.limit,
            hits,
        })
    }
}

pub(crate) fn roots(options: &FileSearchOptions) -> Result<Vec<PathBuf>> {
    let roots = if options.roots.is_empty() {
        vec![std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .context("home directory unavailable")?]
    } else {
        options.roots.clone()
    };
    if roots.len() > 8 {
        bail!("too many search roots");
    }
    roots
        .into_iter()
        .map(|root| {
            if !root.is_absolute() {
                bail!("search directory must be absolute");
            }
            let root = root
                .canonicalize()
                .context("search directory unavailable")?;
            if !root.is_dir() {
                bail!("search root is not a directory");
            }
            Ok(root)
        })
        .collect()
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn searchable_path(path: &Path, include_hidden: bool) -> bool {
    path.is_absolute()
        && searchable_components(path, include_hidden)
}

fn searchable_components(path: &Path, include_hidden: bool) -> bool {
    !path.components().any(|part| {
        let Component::Normal(name) = part else {
            return false;
        };
        let name = name.to_string_lossy();
        (!include_hidden && name.starts_with('.'))
            || NOISE_DIRECTORIES.contains(&name.as_ref())
            || name.ends_with(".app")
            || name.ends_with(".framework")
    })
}

pub(crate) fn accept_path(
    path: &Path,
    options: &FileSearchOptions,
    root: &Path,
) -> Option<FileSearchHit> {
    let path = path.canonicalize().ok()?;
    let relative = path.strip_prefix(root).ok()?;
    if !searchable_components(relative, options.include_hidden) {
        return None;
    }
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() && !metadata.is_dir() {
        return None;
    }
    let relative_directories = if metadata.is_dir() {
        relative
    } else {
        relative.parent().unwrap_or_else(|| Path::new(""))
    };
    if relative_directories.components().any(|part| match part {
        Component::Normal(name) => options
            .excluded_directory_names
            .iter()
            .any(|excluded| name == excluded.as_str()),
        _ => false,
    }) {
        return None;
    }
    Some(FileSearchHit {
        path,
        is_directory: metadata.is_dir(),
    })
}

fn glob_literal(name: &str) -> String {
    let mut escaped = String::new();
    for ch in name.chars() {
        if "\\*?[]{}!".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// GUI launch PATH is resolved by the same tool lookup as developer commands.
pub fn ripgrep(
    query: &str,
    limit: usize,
    options: &FileSearchOptions,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
) -> Result<Vec<FileSearchHit>> {
    if query.len() > 256
        || query.trim().chars().count() < 2
        || query.chars().any(char::is_control)
        || limit == 0
        || cancelled()
    {
        return Ok(Vec::new());
    }
    let program =
        crate::process::find_executable("rg").context("ripgrep is unavailable; install rg")?;
    scan(&program, query, limit.min(64), options, cancelled, timeout)
}

fn scan(
    program: &Path,
    query: &str,
    limit: usize,
    options: &FileSearchOptions,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
) -> Result<Vec<FileSearchHit>> {
    let roots = roots(options)?;
    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let deadline = Instant::now() + timeout;
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    let mut entries = 0usize;
    for root in roots {
        if cancelled() {
            return Ok(Vec::new());
        }
        let mut arguments = vec![
            "--files".into(),
            "--threads".into(),
            "1".into(),
            "--null".into(),
            "--no-config".into(),
            "--no-ignore".into(),
            "--no-messages".into(),
        ];
        if options.include_hidden {
            arguments.push("--hidden".into());
        }
        for name in NOISE_DIRECTORIES
            .iter()
            .copied()
            .chain(options.excluded_directory_names.iter().map(String::as_str))
        {
            arguments.extend(["--glob".into(), format!("!**/{}/**", glob_literal(name))]);
        }
        for extension in ["app", "framework"] {
            arguments.extend(["--glob".into(), format!("!**/*.{extension}/**")]);
        }
        arguments.extend(["--".into(), ".".into()]);
        #[cfg(windows)]
        let work_directory = dunce::simplified(&root).to_path_buf();
        #[cfg(not(windows))]
        let work_directory = root.clone();
        let mut child = OwnedProcess::spawn(&ProcessRequest {
            title: "File search".into(),
            program: program.to_path_buf(),
            arguments,
            directory: work_directory,
            environment: Default::default(),
        })?;
        let mut pending = Vec::new();
        let mut buffer = [0u8; 8192];
        let mut exited: Option<std::process::ExitStatus> = None;
        loop {
            if cancelled() {
                return Ok(Vec::new());
            }
            if Instant::now() >= deadline || total >= MAX_OUTPUT || entries >= MAX_ENTRIES {
                if hits.is_empty() {
                    bail!("file scan limit reached; narrow search directories");
                }
                return Ok(hits);
            }
            let count = child.read_stdout(&mut buffer)?;
            total += count;
            if count > 0 {
                for byte in &buffer[..count] {
                    if *byte == 0 {
                        entries += 1;
                        if let Ok(raw) = std::str::from_utf8(&pending) {
                            let relative = Path::new(raw);
                            // rg --files emits names relative to the configured working directory.
                            if !relative.is_absolute()
                                && !relative
                                    .components()
                                    .any(|p| matches!(p, Component::ParentDir))
                            {
                                if let Some(name) = relative.file_name().and_then(|s| s.to_str()) {
                                    let lower = name.to_lowercase();
                                    if words.iter().all(|word| lower.contains(word)) {
                                        if let Some(hit) =
                                            accept_path(&root.join(relative), options, &root)
                                        {
                                            if !hit.is_directory && seen.insert(hit.path.clone()) {
                                                hits.push(hit);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        pending.clear();
                        if hits.len() >= limit {
                            return Ok(hits);
                        }
                    } else {
                        pending.push(*byte);
                        if pending.len() > MAX_PATH {
                            bail!("file scan path limit reached");
                        }
                    }
                }
            }
            // Drain stderr even for a malformed tool; never block its stdout pipe.
            let _ = child.read_stderr(&mut buffer)?;
            if count == 0 {
                if let Some(status) = exited {
                    if !pending.is_empty() {
                        bail!("incomplete file scan output");
                    }
                    if !status.success() && status.code() != Some(1) && hits.is_empty() {
                        bail!("file scan failed");
                    }
                    break;
                }
                if let Some(status) = child.poll()? {
                    // The process may have written between the empty read and poll.
                    // Once reaped, drain its final pipe bytes before reporting completion.
                    exited = Some(status);
                    continue;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    Ok(hits)
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn ripgrep_backend_finds_matching_files_on_windows_when_rg_is_installed() {
        if !RipgrepBackend.is_available() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let match_file = canonical.join("quarterly_report_2026.txt");
        let ignored_dir = canonical.join("node_modules");
        std::fs::create_dir(&ignored_dir).unwrap();
        std::fs::write(&match_file, "ok").unwrap();
        std::fs::write(ignored_dir.join("quarterly_report_2026.txt"), "noise").unwrap();
        let options = FileSearchOptions {
            roots: vec![canonical.clone()],
            engine: FileSearchEngine::Ripgrep,
            ..Default::default()
        };
        let query = FileSearchQuery {
            text: "quarterly report",
            roots: std::slice::from_ref(&canonical),
            limit: 16,
            options: &options,
            timeout: Duration::from_secs(5),
        };
        let result = RipgrepBackend.search(&query, &|| false).unwrap();
        assert_eq!(
            result.hits,
            vec![FileSearchHit {
                path: match_file,
                is_directory: false,
            }]
        );
    }
}

pub(crate) fn merge(
    index: Result<Vec<FileSearchHit>>,
    scan: Result<Vec<FileSearchHit>>,
    limit: usize,
) -> Result<Vec<FileSearchHit>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let (index, scan) = match (index, scan) {
        (Err(_), Err(error)) => return Err(error),
        (index, scan) => (index.unwrap_or_default(), scan.unwrap_or_default()),
    };
    let mut seen = BTreeSet::new();
    // Interleave so an indexed batch cannot crowd out newly found files.
    let mut index = index.into_iter();
    let mut scan = scan.into_iter();
    let mut hits = Vec::new();
    loop {
        let pair = [index.next(), scan.next()];
        if pair.iter().all(Option::is_none) {
            break;
        }
        for hit in pair.into_iter().flatten() {
            if seen.insert(hit.path.clone()) {
                hits.push(hit);
            }
            if hits.len() >= limit.min(64) {
                return Ok(hits);
            }
        }
    }
    Ok(hits)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(script: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::Builder::new()
            .prefix("anywhere-rg-")
            .tempdir_in("/tmp")
            .unwrap();
        let program = dir.path().join("fake-rg");
        std::fs::write(&program, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, program)
    }

    #[test]
    fn names_are_literal_nul_delimited_and_canonical_targets_stay_in_root() {
        let (dir, program) = fixture("printf '%s\\0' './报告[季度].md' './报告[季度].md' './parent/报告.md' './outside-报告.md' './.报告.env' '../报告.md'");
        std::fs::write(dir.path().join("报告[季度].md"), "no content search").unwrap();
        std::fs::write(dir.path().join(".报告.env"), "secret").unwrap();
        std::fs::create_dir(dir.path().join("parent")).unwrap();
        std::fs::write(dir.path().join("parent/报告.md"), "text").unwrap();
        std::os::unix::fs::symlink("/etc/hosts", dir.path().join("outside-报告.md")).unwrap();
        let mut options = FileSearchOptions {
            roots: vec![dir.path().to_path_buf()],
            excluded_directory_names: vec!["parent".into()],
            ..Default::default()
        };
        let hits = scan(
            &program,
            "报告",
            64,
            &options,
            &|| false,
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        options.include_hidden = true;
        let hits = scan(
            &program,
            "报告",
            64,
            &options,
            &|| false,
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            scan(
                &program,
                "报告[季度]",
                64,
                &options,
                &|| false,
                Duration::from_secs(1)
            )
            .unwrap()
            .len(),
            1
        );
        assert!(scan(
            &program,
            "content",
            64,
            &options,
            &|| false,
            Duration::from_secs(1)
        )
        .unwrap()
        .is_empty());
        assert_eq!(glob_literal("cache[1]*"), r"cache\[1\]\*");
    }

    #[test]
    fn scan_is_cancelled_and_reaped_and_timeout_does_not_block() {
        // Publish readiness atomically: cancellation must not kill the fixture
        // between creating an empty pid file and writing its process identity.
        let (dir, program) =
            fixture("printf '%s' \"$$\" > pid.pending; mv pid.pending pid; exec /bin/sleep 5");
        let options = FileSearchOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        for cancel in [false, true] {
            let pid_file = dir.path().join("pid");
            let _ = std::fs::remove_file(&pid_file);
            let start = Instant::now();
            let result = scan(
                &program,
                "report",
                64,
                &options,
                &|| cancel && pid_file.exists(),
                Duration::from_secs(1),
            );
            if cancel {
                assert!(result.unwrap().is_empty());
            } else {
                assert!(result.is_err());
            }
            assert!(start.elapsed() < Duration::from_secs(2));
            let pid: libc::pid_t = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
            // SAFETY: signal 0 probes only the fixture child; OwnedProcess reaps it.
            unsafe {
                assert_eq!(libc::kill(pid, 0), -1);
            }
        }
    }

    #[test]
    fn result_limit_stops_a_live_scanner() {
        let (dir, program) = fixture("printf '%s' \"$$\" > pid; printf '%s\\0' './report-a' './report-b' './report-c'; exec /bin/sleep 5");
        for name in ["report-a", "report-b", "report-c"] {
            std::fs::write(dir.path().join(name), "data").unwrap();
        }
        let start = Instant::now();
        let hits = scan(
            &program,
            "report",
            2,
            &FileSearchOptions {
                roots: vec![dir.path().to_path_buf()],
                ..Default::default()
            },
            &|| false,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert!(start.elapsed() < Duration::from_secs(2));
        let pid: libc::pid_t = std::fs::read_to_string(dir.path().join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: signal 0 probes only the fixture child.
        unsafe {
            assert_eq!(libc::kill(pid, 0), -1);
        }
    }

    #[test]
    fn merge_deduplicates_and_reserves_room_for_unindexed_files() {
        let hit = |name: &str| FileSearchHit {
            path: name.into(),
            is_directory: false,
        };
        let hits = merge(
            Ok(vec![hit("/a"), hit("/b")]),
            Ok(vec![hit("/a"), hit("/hidden")]),
            3,
        )
        .unwrap();
        assert_eq!(
            hits.iter()
                .map(|h| h.path.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["/a", "/b", "/hidden"]
        );
        assert_eq!(
            merge(
                Err(anyhow::anyhow!("index unavailable")),
                Ok(vec![hit("/hidden")]),
                64
            )
            .unwrap()
            .len(),
            1
        );
        assert!(merge(
            Err(anyhow::anyhow!("index unavailable")),
            Err(anyhow::anyhow!("scan unavailable")),
            64
        )
        .is_err());
    }

    #[test]
    fn malformed_output_and_failures_are_not_successful_empty_results() {
        let (dir, program) = fixture("printf '%s' './unfinished'; exit 2");
        let options = FileSearchOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        assert!(scan(
            &program,
            "report",
            64,
            &options,
            &|| false,
            Duration::from_secs(1)
        )
        .is_err());
        let (dir, program) = fixture("exit 1");
        let options = FileSearchOptions {
            roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        assert!(scan(
            &program,
            "report",
            64,
            &options,
            &|| false,
            Duration::from_secs(1)
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    #[ignore = "requires installed ripgrep"]
    fn real_ripgrep_finds_hidden_and_gitignored_files_without_an_index() {
        let dir = tempfile::Builder::new()
            .prefix("anywhere-real-rg-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.md\n").unwrap();
        std::fs::write(dir.path().join("report.md"), "data").unwrap();
        std::fs::write(dir.path().join(".report.env"), "data").unwrap();
        std::fs::create_dir(dir.path().join(".config")).unwrap();
        std::fs::write(dir.path().join(".config/report.json"), "data").unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("node_modules/report.js"), "data").unwrap();
        let options = FileSearchOptions {
            roots: vec![dir.path().to_path_buf()],
            include_hidden: true,
            ..Default::default()
        };
        let hits = ripgrep("report", 64, &options, &|| false, Duration::from_secs(2)).unwrap();
        assert_eq!(hits.len(), 3, "{hits:?}");
        let hits = ripgrep(
            "report",
            64,
            &FileSearchOptions {
                include_hidden: false,
                ..options
            },
            &|| false,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
    }
}
