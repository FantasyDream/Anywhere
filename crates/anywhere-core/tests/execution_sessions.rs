#![cfg(unix)]
use anywhere_core::core::execution::{
    ExecutionSessionManager, MAX_RUNNING_SESSIONS, MAX_STREAM_BYTES,
};
use anywhere_platform::process::ProcessRequest;
use anywhere_protocol::ExecutionSessionStatus;
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn request(directory: &Path, script: &str) -> ProcessRequest {
    ProcessRequest {
        title: "Test task".into(),
        program: "/bin/sh".into(),
        arguments: vec!["-c".into(), script.into()],
        directory: directory.into(),
    }
}

fn wait(manager: &ExecutionSessionManager, id: &str) -> anywhere_protocol::ExecutionSessionDetail {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let detail = manager.detail(id).unwrap();
        if !detail.summary.status.is_active() {
            return detail;
        }
        assert!(Instant::now() < deadline, "task did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn stdout_stderr_and_nonzero_exit_are_retained_and_arguments_are_literal() {
    let dir = tempfile::tempdir().unwrap();
    let manager = ExecutionSessionManager::default();
    let id = manager
        .start(request(
            dir.path(),
            "printf '中文🙂'; printf 'failure' >&2; exit 7",
        ))
        .unwrap();
    let detail = wait(&manager, &id);
    assert_eq!(detail.summary.status, ExecutionSessionStatus::Failed);
    assert_eq!(detail.summary.exit_code, Some(7));
    assert_eq!(detail.stdout, "中文🙂");
    assert_eq!(detail.stderr, "failure");
    manager.stop(&id).unwrap(); // Completed tasks are never signalled again.
    assert_eq!(manager.detail(&id).unwrap(), detail);
    manager.remove(&id).unwrap();
    assert!(manager.list().is_empty());

    let literal = "$(touch unexpected); `whoami`";
    let id = manager
        .start(ProcessRequest {
            title: "Literal".into(),
            program: "/usr/bin/printf".into(),
            arguments: vec!["%s".into(), literal.into()],
            directory: dir.path().into(),
        })
        .unwrap();
    assert_eq!(wait(&manager, &id).stdout, literal);
    assert!(!dir.path().join("unexpected").exists());
}

#[test]
fn high_output_never_blocks_process_exit_and_tail_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let manager = ExecutionSessionManager::default();
    let id = manager.start(request(dir.path(), "i=0; while [ $i -lt 24000 ]; do printf 'abcdefghijklmno\n'; printf 'stderr012345678\n' >&2; i=$((i+1)); done; printf 'END'; printf 'ERR-END' >&2")).unwrap();
    let detail = wait(&manager, &id);
    assert_eq!(detail.summary.status, ExecutionSessionStatus::Succeeded);
    assert!(detail.summary.output_truncated);
    assert!(detail.stdout.len() <= MAX_STREAM_BYTES);
    assert!(detail.stderr.len() <= MAX_STREAM_BYTES);
    assert!(detail.stdout.ends_with("END"));
    assert!(detail.stderr.ends_with("ERR-END"));
}

#[test]
fn stop_kills_descendants_and_releases_concurrency_slot() {
    let dir = tempfile::tempdir().unwrap();
    let manager = ExecutionSessionManager::default();
    let id = manager
        .start(request(
            dir.path(),
            "(sleep 0.3; printf orphan > orphan) & printf READY; wait",
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !manager.detail(&id).unwrap().stdout.contains("READY") {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(manager.remove(&id).is_err());
    manager.stop(&id).unwrap();
    assert_eq!(
        wait(&manager, &id).summary.status,
        ExecutionSessionStatus::Stopped
    );
    std::thread::sleep(Duration::from_millis(350));
    assert!(!dir.path().join("orphan").exists());
    let ids: Vec<_> = (0..MAX_RUNNING_SESSIONS)
        .map(|_| manager.start(request(dir.path(), "sleep 30")).unwrap())
        .collect();
    assert!(manager.start(request(dir.path(), "sleep 30")).is_err());
    for id in ids {
        manager.stop(&id).unwrap();
        wait(&manager, &id);
    }
    let last = manager.start(request(dir.path(), "printf OK")).unwrap();
    assert_eq!(wait(&manager, &last).stdout, "OK");
    manager.shutdown();
    assert!(manager.list().is_empty());
    assert!(manager.start(request(dir.path(), "printf NO")).is_err());
}

#[test]
fn completed_results_are_evicted_but_running_tasks_are_retained() {
    let dir = tempfile::tempdir().unwrap();
    let manager = ExecutionSessionManager::default();
    let active = manager.start(request(dir.path(), "sleep 30")).unwrap();
    let first = manager.start(request(dir.path(), "printf FIRST")).unwrap();
    wait(&manager, &first);
    for _ in 0..32 {
        let id = manager.start(request(dir.path(), "printf OK")).unwrap();
        wait(&manager, &id);
    }
    assert_eq!(manager.list().len(), 32);
    assert!(manager.detail(&first).is_none());
    assert!(manager.detail(&active).unwrap().summary.status.is_active());
    manager.stop(&active).unwrap();
    wait(&manager, &active);
}
