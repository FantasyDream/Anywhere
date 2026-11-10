#![cfg(test)]

use super::record::MAX_RECORD;
use super::*;
use anywhere_platform::FileSearchHit;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    time::Duration,
};

#[test]
fn pinyin_finds_unindexed_hidden_names_without_widening_the_type_filter() {
    let fixture = tempfile::tempdir().unwrap();
    let hidden = fixture.path().join(".隐藏目录");
    std::fs::create_dir(&hidden).unwrap();
    let file = hidden.join("开发文档.md");
    std::fs::write(&file, "example").unwrap();
    std::fs::create_dir(hidden.join("开发文档")).unwrap();
    for input in ["kaifawendang", "kfwd", "kai 文档"] {
        let store =
            ScanStore::new(input, FileSearchFilter::Files, options(fixture.path())).unwrap();
        let page = finish(&store);
        assert_eq!(page.hits.len(), 1, "{input}");
        assert_eq!(page.hits[0].path, file.canonicalize().unwrap());
    }
    let query = NameQuery::new("kfwd", FileSearchFilter::Pdf);
    assert!(query.rank(&file, false).is_none());
}

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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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

#[test]
fn ordinary_cloud_directory_timeout_still_reports_partial_results() {
    let fixture = tempfile::tempdir().unwrap();
    std::fs::write(fixture.path().join("report.txt"), "").unwrap();
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
    store.io_issue(
        &fixture
            .path()
            .join("Library/CloudStorage/OneDrive/Documents"),
        &std::io::Error::from(std::io::ErrorKind::TimedOut),
    );
    let page = finish(&store);
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 1);
    assert_eq!(page.progress.inaccessible_count, 0);
    assert!(page.progress.issue_examples[0].contains("OneDrive/Documents"));
    assert_eq!(page.hits.len(), 1);
}

#[test]
fn vanished_child_is_skipped_without_hiding_accessible_matches() {
    let fixture = tempfile::tempdir().unwrap();
    std::fs::write(fixture.path().join("report.txt"), "").unwrap();
    let removed = fixture.path().join("updater-temporary-directory");
    std::fs::create_dir(&removed).unwrap();
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
    std::fs::remove_dir(&removed).unwrap();
    let error = std::fs::read_dir(&removed).unwrap_err();
    store.io_issue(&removed, &error);
    let page = finish(&store);
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 0);
    assert_eq!(page.progress.inaccessible_count, 0);
    assert!(page.progress.issue_examples.is_empty());
    assert_eq!(page.hits.len(), 1);
}

#[test]
fn removed_root_reports_its_cause_without_hiding_accessible_matches() {
    let fixture = tempfile::tempdir().unwrap();
    std::fs::write(fixture.path().join("report.txt"), "").unwrap();
    let removed = fixture.path().join("removed");
    std::fs::create_dir(&removed).unwrap();
    let mut policy = options(fixture.path());
    policy.roots.push(removed.clone());
    let store = ScanStore::new("report", FileSearchFilter::All, policy).unwrap();
    std::fs::remove_dir(&removed).unwrap();
    let page = finish(&store);
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 1);
    assert_eq!(page.progress.inaccessible_count, 0);
    assert!(page.progress.issue_examples[0].starts_with(&removed.display().to_string()));
    assert_eq!(page.hits.len(), 1);
    assert_eq!(
        page.hits[0].path,
        fixture.path().join("report.txt").canonicalize().unwrap()
    );
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
    assert!(store
        .run(&|| true, |_| panic!("cancelled query published"))
        .is_err());
    assert!(!store.progress().finished);
    let first = finish(&store);
    let cursor = first.progress.next_cursor.unwrap();
    let another = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
    finish(&another);
    assert!(another.page(Some(&cursor), &|| false).is_err());
    assert!(store.page(Some(&cursor), &|| true).is_err());
    let mut forged: serde_json::Value = serde_json::from_str(&cursor).unwrap();
    forged["page"] = u32::MAX.into();
    assert!(store.page(Some(&forged.to_string()), &|| false).is_err());
}

#[test]
fn home_prefixed_file_search_keeps_the_authorized_scope() {
    let home = anywhere_platform::paths::home_directory().expect("test user's home");
    let fixture = tempfile::tempdir_in(&home).unwrap();
    let file = fixture.path().join("项目 file.txt");
    std::fs::write(&file, "contents").unwrap();
    let mut policy = options(fixture.path());
    policy.restrict_to_roots = true;
    for prefix in ["~", "～"] {
        let input = format!(
            "{prefix}/{}",
            file.strip_prefix(&home).unwrap().to_str().unwrap()
        );
        let store = ScanStore::new(&input, FileSearchFilter::All, policy.clone()).unwrap();
        let page = finish(&store);
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].path, file.canonicalize().unwrap());
        assert!(page.progress.finished && page.progress.issue_count == 0);
        // A home alias does not authorize Home when only the fixture is allowed.
        assert!(ScanStore::new(prefix, FileSearchFilter::All, policy.clone()).is_err());
        assert!(ScanStore::new(
            &format!("{prefix}/../"),
            FileSearchFilter::All,
            policy.clone()
        )
        .is_err());
    }
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
            ScanStore::new(path.to_str().unwrap(), FileSearchFilter::All, options(root)).unwrap();
        let page = finish(&store);
        assert_eq!(page.hits.len(), count);
        assert!(page.progress.finished && page.progress.issue_count == 0);
    }
}

#[test]
fn system_exclusions_are_pruned_from_traversal_and_index_hits() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().canonicalize().unwrap();
    let temporary = root.join("system-temp");
    let caches = root.join("Library/Caches");
    let paths = [
        root.join("report.txt"),
        root.join("project/tmp/report.txt"),
        root.join(".m2/report.txt"),
        root.join(".cache/report.txt"),
        caches.join("ordinary/report.txt"),
        root.join("Library/CloudStorage/OneDrive-个人/Documents/report.txt"),
        root.join("Library/CloudStorage/OneDrive-个人/project/.Trash/report.txt"),
        root.join("project/.Trash/report.txt"),
        root.join(".Trash/report.txt"),
        root.join("Library/CloudStorage/OneDrive-个人/.Trash/report.txt"),
        temporary.join("report.txt"),
        caches.join("example.ShipIt/update.123/report.txt"),
    ];
    for path in &paths {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    let mut store = ScanStore::new("report", FileSearchFilter::All, options(&root)).unwrap();
    store.scope.system_exclusions =
        anywhere_platform::search_exclusions::SearchExclusions::with_locations(
            std::slice::from_ref(&root),
            vec![temporary],
            Some(root.clone()),
        );
    let hits = paths
        .iter()
        .cloned()
        .map(|path| FileSearchHit {
            path,
            is_directory: false,
        })
        .collect();
    let mut indexed: Vec<_> = store
        .verify_index_hits(hits, &|| false)
        .into_iter()
        .map(|hit| hit.path)
        .collect();
    let page = finish(&store);
    let mut scanned: Vec<_> = page.hits.iter().map(|hit| hit.path.clone()).collect();
    let mut expected = paths[..8].to_vec();
    indexed.sort();
    scanned.sort();
    expected.sort();
    assert_eq!(indexed, expected);
    assert_eq!(scanned, expected);
    assert_eq!(page.progress.issue_count, 0);
    for selected in [
        root.join("system-temp"),
        root.join(".Trash"),
        root.join("Library/CloudStorage/OneDrive-个人/.Trash"),
    ] {
        let mut explicit = ScanStore::new(
            selected.to_str().unwrap(),
            FileSearchFilter::All,
            options(&root),
        )
        .unwrap();
        explicit.scope.system_exclusions =
            anywhere_platform::search_exclusions::SearchExclusions::with_locations(
                &explicit.scope.options.roots,
                vec![root.join("system-temp")],
                Some(root.clone()),
            );
        let page = finish(&explicit);
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].path, selected.join("report.txt"));
        assert_eq!(page.progress.issue_count, 0);
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("report", FileSearchFilter::All, options(fixture.path())).unwrap();
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
    let store = ScanStore::new("needle", FileSearchFilter::Pdf, options(fixture.path())).unwrap();
    let page = finish(&store);
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 0);
    assert_eq!(page.progress.scanned_entries, 50_101);
    assert_eq!(page.hits.len(), 1);
}

#[cfg(unix)]
#[test]
fn retargeted_links_are_rechecked_by_index_and_traversal() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = fixture.path().canonicalize().unwrap();
    let target = root.join("report-original.txt");
    let foreign = outside.path().join("report-outside.txt");
    std::fs::write(&target, "synthetic").unwrap();
    std::fs::write(&foreign, "synthetic").unwrap();
    let link = root.join("report-link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut policy = options(&root);
    policy.restrict_to_roots = true;
    let store = ScanStore::new("report", FileSearchFilter::All, policy).unwrap();
    let indexed = || {
        vec![FileSearchHit {
            path: link.clone(),
            is_directory: false,
        }]
    };
    assert_eq!(store.verify_index_hits(indexed(), &|| false).len(), 1);
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&foreign, &link).unwrap();
    assert!(store.verify_index_hits(indexed(), &|| false).is_empty());
    let page = finish(&store);
    assert_eq!(
        page.hits
            .iter()
            .map(|hit| hit.path.clone())
            .collect::<Vec<_>>(),
        vec![target]
    );
    assert!(page.progress.finished);
    assert_eq!(page.progress.issue_count, 0);
}

#[test]
fn path_records_preserve_boundaries_and_reject_truncated_tails() {
    let paths = [
        (PathBuf::from("合成 document.txt"), false),
        (PathBuf::from("nested/目录"), true),
    ];
    let mut complete = Vec::new();
    let mut boundaries = Vec::new();
    for (path, directory) in &paths {
        boundaries.push(write_record(&mut complete, path, *directory).unwrap());
    }
    let mut reader = complete.as_slice();
    for ((path, directory), bytes) in paths.iter().zip(&boundaries) {
        let (decoded, kind, consumed) = read_record(&mut reader).unwrap().unwrap();
        assert_eq!((&decoded, kind, consumed), (path, *directory, *bytes));
    }
    assert!(read_record(&mut reader).unwrap().is_none());
    let first = boundaries[0] as usize;
    for cut in first + 1..complete.len() {
        let mut reader = &complete[..cut];
        assert!(read_record(&mut reader).unwrap().is_some());
        assert!(read_record(&mut reader).is_err(), "truncated tail at {cut}");
    }
    let mut oversized = ((MAX_RECORD + 2) as u32).to_le_bytes().to_vec();
    oversized.push(0);
    assert!(read_record(&mut oversized.as_slice()).is_err());
}
