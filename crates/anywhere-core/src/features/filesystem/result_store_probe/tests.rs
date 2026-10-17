use super::{
    append,
    sqlite::{path_key, SqliteResults},
    store,
};
use crate::features::filesystem::scan::{RankedHit, ScanStore, PAGE_SIZE};
use std::{
    cell::Cell,
    collections::HashSet,
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};

fn paths(count: usize) -> Vec<(PathBuf, bool)> {
    (0..count)
        .rev()
        .map(|i| {
            (
                PathBuf::from(format!("/probe/{}/report-{i:05}.txt", i % 19)),
                i % 7 == 0,
            )
        })
        .collect()
}

fn drain(
    store: &ScanStore,
    sqlite: &mut SqliteResults,
    displayed: &mut HashSet<PathBuf>,
) -> Vec<RankedHit> {
    let mut results = Vec::new();
    loop {
        let batch = sqlite.more(store, displayed, &|| false).unwrap();
        assert!(batch.len() <= PAGE_SIZE);
        if batch.is_empty() {
            break;
        }
        for hit in batch {
            assert!(displayed.insert(hit.path.clone()), "duplicate result");
            results.push(hit);
        }
    }
    results
}

#[test]
fn path_keys_follow_unix_path_comparison_including_component_prefixes() {
    let mut paths: Vec<PathBuf> = [
        "/",
        ".",
        "..",
        "a",
        "a/b",
        "a-b",
        "/a/report",
        "/a-b/report",
        "/a//report",
        "/a/./report",
        "/中文/report",
        "/😀/report",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    for byte in 1..=255 {
        if byte != b'/' {
            paths.push(
                Path::new("/probe")
                    .join(OsString::from_vec(vec![byte]))
                    .join("report"),
            );
        }
    }
    for a in &paths {
        for b in &paths {
            assert_eq!(
                a.cmp(b),
                path_key(a).cmp(&path_key(b)),
                "{a:?} versus {b:?}"
            );
        }
    }
}

#[test]
fn identical_raw_records_have_identical_complete_order_and_identity() {
    let store = store();
    let mut input = paths(17001);
    for parent in ["a", "a-b", "中文", "😀", "é", "e\u{301}"] {
        for name in [
            "report",
            "REPORT",
            "report-中文",
            "é-report",
            "e\u{301}-report",
        ] {
            input.push((Path::new("/probe").join(parent).join(name), false));
        }
    }
    for byte in [0x80, 0xff] {
        input.push((
            Path::new("/probe")
                .join(OsString::from_vec(vec![byte]))
                .join("report"),
            false,
        ));
    }
    append(&store, input.iter().cloned());
    let mut expected: Vec<_> = input
        .into_iter()
        .map(|(p, d)| store.query.rank(&p, d).unwrap())
        .collect();
    expected.sort_by(RankedHit::compare);
    let mut sqlite = SqliteResults::new(&store).unwrap();
    let actual = drain(&store, &mut sqlite, &mut HashSet::new());
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.is_directory, expected.is_directory);
        assert_eq!(actual.score, expected.score);
        assert_eq!(actual.normalized, expected.normalized);
    }
    let expected: Vec<_> = expected.into_iter().map(|hit| hit.path).collect();
    let mut production = Vec::new();
    let mut displayed = HashSet::new();
    loop {
        let batch = store.more(&displayed, &|| false).unwrap();
        if batch.hits.is_empty() {
            break;
        }
        for hit in batch.hits {
            assert!(displayed.insert(hit.path.clone()));
            production.push(hit.path);
        }
    }
    assert_eq!(production, expected);
}

#[test]
fn late_high_ranked_records_do_not_reorder_prefix_or_skip_unread_results() {
    let store = store();
    let input = paths(200);
    append(&store, input.iter().cloned());
    let mut sqlite = SqliteResults::new(&store).unwrap();
    // Model an already shown provisional batch shared by both backends.
    let mut displayed: HashSet<_> = input.iter().take(11).map(|(p, _)| p.clone()).collect();
    let first = sqlite.more(&store, &displayed, &|| false).unwrap();
    let production = store.more(&displayed, &|| false).unwrap();
    assert_eq!(
        first.iter().map(|h| &h.path).collect::<Vec<_>>(),
        production.hits.iter().map(|h| &h.path).collect::<Vec<_>>()
    );
    displayed.extend(first.into_iter().map(|h| h.path));
    let prefix = displayed.clone();
    let late = PathBuf::from("/probe/report");
    append(&store, [(late.clone(), false)]);
    let remaining = drain(&store, &mut sqlite, &mut displayed);
    assert_eq!(remaining[0].path, late);
    let mut production_displayed = prefix.clone();
    let mut production_tail = Vec::new();
    loop {
        let batch = store.more(&production_displayed, &|| false).unwrap();
        if batch.hits.is_empty() {
            break;
        }
        for hit in batch.hits {
            assert!(production_displayed.insert(hit.path.clone()));
            production_tail.push(hit.path);
        }
    }
    assert_eq!(
        remaining
            .into_iter()
            .map(|hit| hit.path)
            .collect::<Vec<_>>(),
        production_tail
    );
    assert!(prefix.is_subset(&displayed));
    assert_eq!(displayed.len(), 201);
}

#[test]
fn cancellation_rolls_back_ingestion_and_consumption_without_lost_rows() {
    let store = store();
    append(&store, paths(1600));
    let mut sqlite = SqliteResults::new(&store).unwrap();
    let checks = Cell::new(0);
    // Fail in a later ingestion transaction, after an earlier batch committed.
    assert!(sqlite
        .more(&store, &HashSet::new(), &|| {
            checks.set(checks.get() + 1);
            checks.get() > 600
        })
        .is_err());
    let first = sqlite.more(&store, &HashSet::new(), &|| false).unwrap();
    let mut displayed: HashSet<_> = first.into_iter().map(|h| h.path).collect();
    checks.set(0);
    assert!(sqlite
        .more(&store, &displayed, &|| {
            checks.set(checks.get() + 1);
            checks.get() > 10
        })
        .is_err());
    drain(&store, &mut sqlite, &mut displayed);
    assert_eq!(displayed.len(), 1600);
}

#[test]
fn empty_and_repeated_source_records_are_safe_before_and_after_delivery() {
    let store = store();
    let mut sqlite = SqliteResults::new(&store).unwrap();
    let mut displayed = HashSet::new();
    assert!(drain(&store, &mut sqlite, &mut displayed).is_empty());
    let input = paths(100);
    append(&store, input.iter().cloned().chain(input.iter().cloned()));
    drain(&store, &mut sqlite, &mut displayed);
    assert_eq!(displayed.len(), 100);
    append(&store, input);
    assert!(drain(&store, &mut sqlite, &mut displayed).is_empty());
    assert_eq!(displayed.len(), 100);
}

#[test]
fn disk_full_and_readonly_failures_can_retry_without_omissions() {
    let store = store();
    append(&store, paths(2000));
    let mut sqlite = SqliteResults::new(&store).unwrap();
    let pages: i64 = sqlite
        .connection
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    sqlite
        .connection
        .pragma_update(None, "max_page_count", pages + 1)
        .unwrap();
    let error = sqlite.more(&store, &HashSet::new(), &|| false).unwrap_err();
    assert!(
        matches!(error.downcast_ref::<rusqlite::Error>(), Some(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::DiskFull)
    );
    sqlite
        .connection
        .pragma_update(None, "max_page_count", 1_000_000)
        .unwrap();
    let first = sqlite.more(&store, &HashSet::new(), &|| false).unwrap();
    let mut displayed: HashSet<_> = first.into_iter().map(|h| h.path).collect();
    sqlite
        .connection
        .pragma_update(None, "query_only", true)
        .unwrap();
    assert!(sqlite.more(&store, &displayed, &|| false).is_err());
    sqlite
        .connection
        .pragma_update(None, "query_only", false)
        .unwrap();
    drain(&store, &mut sqlite, &mut displayed);
    assert_eq!(displayed.len(), 2000);
}

#[test]
fn query_isolation_cleanup_and_indexed_read_are_explicit() {
    let store = store();
    let mut sqlite = SqliteResults::new(&store).unwrap();
    let other = super::store();
    assert!(sqlite.more(&other, &HashSet::new(), &|| false).is_err());
    let plan: String = sqlite.connection.query_row("EXPLAIN QUERY PLAN SELECT path,directory FROM unread ORDER BY score DESC,normalized,path_key LIMIT 64", [], |row| row.get(3)).unwrap();
    assert!(plan.contains("unread_order"), "{plan}");
    assert!(!plan.contains("TEMP B-TREE"));
    let workspace = store.workspace().to_owned();
    drop(sqlite);
    drop(store);
    assert!(!workspace.exists());
}
