use super::support::*;

async fn completed_task(app: &Application, id: &str) -> ExecutionSessionDetail {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let detail = app.execution_session(id).unwrap();
            if !detail.summary.status.is_active() {
                return detail;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("task did not finish")
}

async fn execute_action(app: &Application, state: &LauncherState, command: &str) -> ActionResult {
    let candidate = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(command))
        .unwrap_or_else(|| panic!("missing {command}"));
    app.execute(state.session_id, state.revision, candidate.id.clone())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cargo_action_uses_the_graph_scope_history_and_independent_execution_lifetime() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'anywhere-session-fixture'\nversion = '0.1.0'\nedition = '2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn answer() -> u32 { 42 }\n").unwrap();
    std::fs::write(
        root.join("build.rs"),
        "fn main() { std::thread::sleep(std::time::Duration::from_millis(200)); }\n",
    )
    .unwrap();
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        Arc::new(TestPlatform::new(root.clone(), true)),
        registry(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(state
        .context_summary
        .iter()
        .any(|context| context.type_id == PROJECT_RUST));
    assert!(state.scopes.iter().any(|scope| scope.id == "rust"));
    let check = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("developer.cargo.check"))
        .unwrap();
    let result = app
        .execute(state.session_id, state.revision, check.id.clone())
        .await
        .unwrap();
    assert!(result.succeeded(), "{:?}", result.error);
    let id = result.execution_session_id.unwrap();
    assert_ne!(id, state.session_id.to_string());
    app.deactivate(state.session_id).unwrap();
    assert!(!app.execution_sessions().is_empty());
    // A later Launcher activation must neither cancel nor replace the started task.
    let next = app.activate().await.unwrap().unwrap();
    assert_ne!(next.session_id, state.session_id);
    let detail = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let detail = app.execution_session(&id).unwrap();
            if !detail.summary.status.is_active() {
                break detail;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        detail.summary.status,
        ExecutionSessionStatus::Succeeded,
        "{}",
        detail.stderr
    );
    assert_eq!(detail.summary.exit_code, Some(0));
    assert_eq!(detail.summary.directory, root.to_string_lossy());
    assert_eq!(detail.summary.command[1..], ["check", "--color", "never"]);
    assert!(detail.stderr.contains("Finished"));
    let usage = history.lock().unwrap().load("com.apple.finder").unwrap();
    assert_eq!(
        usage[&("developer.cargo.check".into(), PROJECT_RUST.into())].execute_count,
        1
    );
    app.shutdown_execution_sessions();
}

#[tokio::test]
async fn project_without_manifest_has_no_rust_scope_and_deleted_manifest_cannot_execute() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(!state.scopes.iter().any(|scope| scope.id == "rust"));
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    app.update_input(state.session_id, root.to_string_lossy().into_owned(), 1)
        .await
        .unwrap();
    let state = settled(&app, state.session_id, 1).await;
    let check = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("developer.cargo.check"))
        .unwrap();
    std::fs::remove_file(root.join("Cargo.toml")).unwrap();
    let result = app
        .execute(state.session_id, 1, check.id.clone())
        .await
        .unwrap();
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
    assert!(app.execution_sessions().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_types_are_additive_and_wrappers_use_the_module_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir
        .path()
        .canonicalize()
        .unwrap()
        .join("project ' with spaces");
    let module = root.join("module/src");
    std::fs::create_dir_all(&module).unwrap();
    for marker in ["Cargo.toml", "pom.xml", "build.gradle.kts"] {
        std::fs::write(root.join(marker), "metadata only").unwrap();
    }
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"echo not-run-during-recognition"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("mvnw"),
        "#!/bin/sh\nprintf 'MAVEN\\n'; printf '%s\\n' \"$@\"; pwd\n",
    )
    .unwrap();
    std::fs::write(
        root.join("gradlew"),
        "#!/bin/sh\nprintf 'GRADLE\\n'; printf '%s\\n' \"$@\"; pwd\n",
    )
    .unwrap();
    let app = app(Arc::new(TestPlatform::new(module, true)));
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    for kind in [PROJECT_RUST, PROJECT_MAVEN, PROJECT_GRADLE, PROJECT_NODE] {
        assert!(state
            .context_summary
            .iter()
            .any(|context| context.type_id == kind));
    }
    for scope in ["rust", "maven", "gradle", "node"] {
        assert!(state.scopes.iter().any(|entry| entry.id == scope));
    }
    assert!(
        app.execution_sessions().is_empty(),
        "recognition must not execute anything"
    );
    for (command, arguments) in [
        (
            "developer.maven.test",
            "--batch-mode\n--no-transfer-progress\ntest",
        ),
        (
            "developer.maven.package",
            "--batch-mode\n--no-transfer-progress\npackage",
        ),
        (
            "developer.gradle.build",
            "--no-daemon\n--console=plain\nbuild",
        ),
        (
            "developer.gradle.test",
            "--no-daemon\n--console=plain\ntest",
        ),
    ] {
        let result = execute_action(&app, &state, command).await;
        assert!(result.succeeded(), "{:?}", result.error);
        let detail = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
        assert_eq!(
            detail.summary.status,
            ExecutionSessionStatus::Succeeded,
            "{}",
            detail.stderr
        );
        assert!(detail.stdout.contains(arguments));
        assert_eq!(detail.summary.directory, root.to_string_lossy());
        assert!(detail.stdout.contains(root.to_str().unwrap()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires npm and Node; run separately as real-tool acceptance"]
async fn npm_declared_scripts_execute_in_sessions_and_missing_scripts_stay_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("package.json"), r#"{"name":"anywhere-session-fixture","scripts":{"build":"node build.js", "test":"node --test test.cjs"}}"#).unwrap();
    std::fs::write(
        root.join("build.js"),
        "require('node:fs').writeFileSync('built.txt', 'built'); console.log('BUILD-DONE')",
    )
    .unwrap();
    std::fs::write(root.join("test.cjs"), "const test=require('node:test'); const assert=require('node:assert/strict'); test('answer',()=>assert.equal(42,42));").unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    for missing in [
        "developer.node.dev",
        "developer.node.start",
        "developer.node.lint",
    ] {
        assert!(!state
            .context_candidates
            .iter()
            .chain(state.candidates.iter())
            .any(|c| c.command_id.as_deref() == Some(missing)));
    }
    assert!(!root.join("built.txt").exists());
    for command in ["developer.node.build", "developer.node.test"] {
        let result = execute_action(&app, &state, command).await;
        assert!(result.succeeded(), "{:?}", result.error);
        let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
        assert_eq!(
            task.summary.status,
            ExecutionSessionStatus::Succeeded,
            "{}",
            task.stderr
        );
        assert_eq!(task.summary.exit_code, Some(0));
    }
    assert_eq!(
        std::fs::read_to_string(root.join("built.txt")).unwrap(),
        "built"
    );
    // Do not let npm's implicit start/test defaults run after the explicit script disappears.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"anywhere-session-fixture","scripts":{}}"#,
    )
    .unwrap();
    let result = execute_action(&app, &state, "developer.node.test").await;
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
    app.update_input(state.session_id, "npm test".into(), 1)
        .await
        .unwrap();
    let refreshed = settled(&app, state.session_id, 1).await;
    assert!(!refreshed.scopes.iter().any(|scope| scope.id == "node"));
    assert!(!refreshed
        .context_candidates
        .iter()
        .chain(refreshed.candidates.iter())
        .any(|c| c
            .command_id
            .as_deref()
            .is_some_and(|id| id.starts_with("developer.node."))));
}

#[tokio::test]
async fn missing_node_scripts_stay_hidden_without_running_tools() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"echo example"}}"#,
    )
    .unwrap();
    let app = Application::new(
        Arc::new(TestPlatform::new(root.clone(), true)),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(state
        .context_candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("developer.node.build")));
    assert!(!state
        .context_candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("developer.node.test")));
    std::fs::write(root.join("package.json"), "{}").unwrap();
    app.update_input(state.session_id, "npm test".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, state.session_id, 1).await;
    assert!(!state.scopes.iter().any(|scope| scope.id == "node"));
    assert!(!state
        .context_candidates
        .iter()
        .chain(state.candidates.iter())
        .any(|c| c
            .command_id
            .as_deref()
            .is_some_and(|id| id.starts_with("developer.node."))));
    assert!(app.execution_sessions().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Maven and Gradle; run separately as real-tool acceptance"]
async fn real_maven_and_gradle_actions_complete_in_process_sessions() {
    let maven = anywhere_platform::process::find_executable("mvn").expect("Maven is required");
    let gradle = std::env::var_os("ANYWHERE_TEST_GRADLE_EXECUTABLE")
        .map(PathBuf::from)
        .or_else(|| anywhere_platform::process::find_executable("gradle"))
        .expect("Gradle is required");
    for (kind, tool, marker, wrapper, commands) in [
        (
            PROJECT_MAVEN,
            maven,
            "pom.xml",
            "mvnw",
            ["developer.maven.test", "developer.maven.package"],
        ),
        (
            PROJECT_GRADLE,
            gradle,
            "build.gradle",
            "gradlew",
            ["developer.gradle.build", "developer.gradle.test"],
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let manifest = if kind == PROJECT_MAVEN {
            r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>test.anywhere</groupId><artifactId>session-fixture</artifactId><version>1.0</version><properties><maven.compiler.source>8</maven.compiler.source><maven.compiler.target>8</maven.compiler.target></properties><dependencies><dependency><groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version><scope>test</scope></dependency></dependencies></project>"#
        } else {
            "plugins { id 'java' }\nrepositories { mavenCentral() }\ndependencies { testImplementation 'junit:junit:4.13.2' }\ntest { useJUnit() }\n"
        };
        std::fs::write(root.join(marker), manifest).unwrap();
        if kind == PROJECT_GRADLE {
            std::fs::write(
                root.join("settings.gradle"),
                "rootProject.name = 'session-fixture'\n",
            )
            .unwrap();
        }
        std::fs::create_dir_all(root.join("src/main/java")).unwrap();
        std::fs::write(
            root.join("src/main/java/Answer.java"),
            "public class Answer { public static int answer() { return 42; } }\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("src/test/java")).unwrap();
        let test_source = "import org.junit.Test; import static org.junit.Assert.*; public class AnswerTest { @Test public void answer() { assertEquals(42, Answer.answer()); } }";
        std::fs::write(root.join("src/test/java/AnswerTest.java"), test_source).unwrap();
        // Forward the project wrapper to a real installed tool; no mocked build output.
        let quoted = tool.to_string_lossy().replace('\'', "'\"'\"'");
        std::fs::write(
            root.join(wrapper),
            format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n"),
        )
        .unwrap();
        let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
        let initial = app.activate().await.unwrap().unwrap();
        let state = settled(&app, initial.session_id, 0).await;
        for command in commands {
            let result = execute_action(&app, &state, command).await;
            assert!(result.succeeded(), "{:?}", result.error);
            let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
            assert_eq!(
                task.summary.status,
                ExecutionSessionStatus::Succeeded,
                "{}\n{}",
                task.stdout,
                task.stderr
            );
            assert_eq!(task.summary.exit_code, Some(0));
        }
        std::fs::write(
            root.join("src/test/java/AnswerTest.java"),
            test_source.replace("assertEquals(42,", "assertEquals(41,"),
        )
        .unwrap();
        let test_command = if kind == PROJECT_MAVEN {
            "developer.maven.test"
        } else {
            "developer.gradle.test"
        };
        let result = execute_action(&app, &state, test_command).await;
        let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
        assert_eq!(
            task.summary.status,
            ExecutionSessionStatus::Failed,
            "{}\n{}",
            task.stdout,
            task.stderr
        );
        assert_ne!(task.summary.exit_code, Some(0));
        assert!(task.stdout.contains("fail") || task.stderr.contains("fail"));
        assert!(if kind == PROJECT_MAVEN {
            root.join("target/session-fixture-1.0.jar").is_file()
        } else {
            root.join("build/classes/java/main/Answer.class").is_file()
        });
    }
}

#[tokio::test]
async fn malformed_node_manifest_does_not_break_other_project_contexts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("pom.xml"), "metadata").unwrap();
    std::fs::write(root.join("package.json"), "invalid").unwrap();
    let app = app(Arc::new(TestPlatform::new(root, true)));
    let initial = app.activate().await.unwrap().unwrap();
    let state = settled(&app, initial.session_id, 0).await;
    assert!(state.scopes.iter().any(|scope| scope.id == "maven"));
    assert!(!state.scopes.iter().any(|scope| scope.id == "node"));
}

#[tokio::test]
async fn dynamic_script_actions_are_searchable_stable_and_revalidate_manager() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"echo build", "check:types":"echo private-body"}}"#,
    )
    .unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let activation = app.activate().await.unwrap().unwrap();
    let state = settled(&app, activation.session_id, 0).await;
    let action = state
        .context_candidates
        .iter()
        .find(|candidate| candidate.title == "npm Run · check:types")
        .unwrap()
        .clone();
    assert!(!format!("{:?}", state.context_summary).contains("private-body"));
    app.update_input(state.session_id, "check:types".into(), 1)
        .await
        .unwrap();
    let search = settled(&app, state.session_id, 1).await;
    assert!(search
        .candidates
        .iter()
        .any(|candidate| candidate.command_id == action.command_id));
    assert!(app.execution_sessions().is_empty());
    app.deactivate(state.session_id).unwrap();
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"unrelated":"echo new", "check:types":"echo changed"}}"#,
    )
    .unwrap();
    let activation = app.activate().await.unwrap().unwrap();
    let state = settled(&app, activation.session_id, 0).await;
    assert!(state
        .context_candidates
        .iter()
        .any(|candidate| candidate.command_id == action.command_id));
    std::fs::write(
        root.join("package.json"),
        r#"{"packageManager":"pnpm@10.0.0", "scripts":{"check:types":"echo changed"}}"#,
    )
    .unwrap();
    let result = execute_action(&app, &state, action.command_id.as_deref().unwrap()).await;
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
    assert!(app.execution_sessions().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cargo_workspace_run_targets_and_actual_test_failures_use_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=['one','two']\nresolver='2'\n",
    )
    .unwrap();
    for package in ["one", "two"] {
        let path = root.join(package);
        std::fs::create_dir_all(path.join("src")).unwrap();
        std::fs::write(
            path.join("Cargo.toml"),
            format!("[package]\nname='{package}'\nversion='0.1.0'\nedition='2021'\n"),
        )
        .unwrap();
        std::fs::write(path.join("src/main.rs"), format!("fn main() {{ println!(\"RUN-{package}\"); }}\n#[test] fn answer() {{ assert_eq!(42,42); }}\n")).unwrap();
    }
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let activation = app.activate().await.unwrap().unwrap();
    let state = settled(&app, activation.session_id, 0).await;
    for package in ["one", "two"] {
        let candidate = state
            .context_candidates
            .iter()
            .find(|c| c.title == format!("Cargo Run · {package} / {package}"))
            .unwrap();
        let result = execute_action(&app, &state, candidate.command_id.as_deref().unwrap()).await;
        let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
        assert_eq!(
            task.summary.status,
            ExecutionSessionStatus::Succeeded,
            "{}",
            task.stderr
        );
        assert!(task.stdout.contains(&format!("RUN-{package}")));
    }
    let result = execute_action(&app, &state, "developer.cargo.test").await;
    assert_eq!(
        completed_task(&app, result.execution_session_id.as_deref().unwrap())
            .await
            .summary
            .status,
        ExecutionSessionStatus::Succeeded
    );
    std::fs::write(
        root.join("two/src/main.rs"),
        "fn main() {}\n#[test] fn fails() { assert_eq!(42,41); }\n",
    )
    .unwrap();
    let result = execute_action(&app, &state, "developer.cargo.test").await;
    let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
    assert_eq!(task.summary.status, ExecutionSessionStatus::Failed);
    assert_ne!(task.summary.exit_code, Some(0));
    assert!(task.stdout.contains("FAILED"));
    std::fs::remove_file(root.join("two/src/main.rs")).unwrap();
    let candidate = state
        .context_candidates
        .iter()
        .find(|c| c.title == "Cargo Run · two / two")
        .unwrap();
    let result = execute_action(&app, &state, candidate.command_id.as_deref().unwrap()).await;
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
}

#[tokio::test]
async fn spring_boot_context_and_run_actions_require_active_local_plugins() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(root.join("pom.xml"), "<project><build><plugins><plugin><groupId>org.springframework.boot</groupId><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build></project>").unwrap();
    std::fs::write(root.join("mvnw"), "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    let app = app(Arc::new(TestPlatform::new(root.clone(), true)));
    let activation = app.activate().await.unwrap().unwrap();
    let state = settled(&app, activation.session_id, 0).await;
    assert!(state
        .context_summary
        .iter()
        .any(|c| c.type_id == JAVA_SPRING_BOOT));
    assert!(state.scopes.iter().any(|c| c.id == "spring_boot"));
    assert!(!state
        .context_candidates
        .iter()
        .any(|c| c.command_id.as_deref() == Some("developer.spring.gradle.run")));
    let result = execute_action(&app, &state, "developer.spring.maven.run").await;
    let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
    assert!(task.stdout.contains("spring-boot:run"));
    std::fs::write(root.join("pom.xml"), "<project/>").unwrap();
    let result = execute_action(&app, &state, "developer.spring.maven.run").await;
    assert_eq!(result.status, ActionStatus::Failed);
    assert!(result.execution_session_id.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Node/npm/Corepack and cached or downloadable pnpm/yarn"]
async fn all_node_managers_execute_custom_scripts_failures_and_stoppable_services() {
    for (manager, version) in [("npm", "11.0.0"), ("pnpm", "10.0.0"), ("yarn", "4.9.2")] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let manifest = serde_json::json!({"name":"anywhere-node-fixture", "version":"1.0.0", "packageManager":format!("{manager}@{version}"),
            "scripts":{"check:types":"node ok.cjs", "lint":"node fail.cjs", "dev":"node service.cjs", "start":"node service.cjs"}});
        std::fs::write(root.join("package.json"), manifest.to_string()).unwrap();
        std::fs::write(root.join("ok.cjs"), "console.log('CUSTOM-SUCCESS')\n").unwrap();
        std::fs::write(
            root.join("fail.cjs"),
            "console.error('EXPECTED-FAILURE');process.exit(13)\n",
        )
        .unwrap();
        std::fs::write(
            root.join("service.cjs"),
            "console.log('SERVICE-PID:'+process.pid);setInterval(()=>{},1000)\n",
        )
        .unwrap();
        if manager == "yarn" {
            let corepack = anywhere_platform::process::find_executable("corepack")
                .expect("Corepack required for Yarn acceptance");
            let node = anywhere_platform::process::find_executable("node").unwrap();
            let path =
                std::env::join_paths(std::iter::once(node.parent().unwrap().to_path_buf()).chain(
                    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
                ))
                .unwrap();
            let output = std::process::Command::new(corepack)
                .args(["yarn", "install"])
                .current_dir(&root)
                .env("PATH", path)
                .env("COREPACK_ENABLE_AUTO_PIN", "0")
                .env("COREPACK_ENABLE_DOWNLOAD_PROMPT", "0")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let app = app(Arc::new(TestPlatform::new(root, true)));
        let activation = app.activate().await.unwrap().unwrap();
        let state = settled(&app, activation.session_id, 0).await;
        for (script, status, code) in [
            ("check:types", ExecutionSessionStatus::Succeeded, 0),
            ("lint", ExecutionSessionStatus::Failed, 13),
        ] {
            let id = anywhere_core::features::developer::node::action_id(manager, script);
            let result = execute_action(&app, &state, &id).await;
            assert!(result.succeeded(), "{:?}", result.error);
            let task = completed_task(&app, result.execution_session_id.as_deref().unwrap()).await;
            assert_eq!(
                task.summary.status, status,
                "{manager}: {}\\n{}",
                task.stdout, task.stderr
            );
            assert_eq!(task.summary.exit_code, Some(code), "{}", task.stderr);
        }
        for script in ["dev", "start"] {
            let result = execute_action(
                &app,
                &state,
                &anywhere_core::features::developer::node::action_id(manager, script),
            )
            .await;
            let id = result.execution_session_id.unwrap();
            let pid = tokio::time::timeout(Duration::from_secs(60), async {
                loop {
                    let task = app.execution_session(&id).unwrap();
                    if let Some(pid) = task.stdout.lines().find_map(|line| {
                        line.strip_prefix("SERVICE-PID:")
                            .and_then(|pid| pid.parse::<u32>().ok())
                    }) {
                        break pid;
                    }
                    assert!(
                        task.summary.status.is_active(),
                        "{manager}: {}\\n{}",
                        task.stdout,
                        task.stderr
                    );
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            app.stop_execution_session(&id).unwrap();
            assert_eq!(
                completed_task(&app, &id).await.summary.status,
                ExecutionSessionStatus::Stopped
            );
            assert!(
                !std::process::Command::new("/bin/kill")
                    .args(["-0", &pid.to_string()])
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "service survived Stop"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Java/Maven/Gradle and cached or downloadable Spring Boot dependencies"]
async fn real_spring_boot_services_start_and_stop_without_surviving_children() {
    let maven = anywhere_platform::process::find_executable("mvn").expect("Maven required");
    let gradle = std::env::var_os("ANYWHERE_TEST_GRADLE_EXECUTABLE")
        .map(PathBuf::from)
        .or_else(|| anywhere_platform::process::find_executable("gradle"))
        .expect("Gradle required");
    for (kind, tool, wrapper, marker, command) in [
        (
            PROJECT_MAVEN,
            maven,
            "mvnw",
            "pom.xml",
            "developer.spring.maven.run",
        ),
        (
            PROJECT_GRADLE,
            gradle,
            "gradlew",
            "build.gradle",
            "developer.spring.gradle.run",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let manifest = if kind == PROJECT_MAVEN {
            r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId><version>4.1.1</version><relativePath/></parent><groupId>test.anywhere</groupId><artifactId>boot-fixture</artifactId><version>1.0</version><properties><java.version>17</java.version></properties><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId></dependency></dependencies><build><plugins><plugin><groupId>org.springframework.boot</groupId><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build></project>"#
        } else {
            "plugins { id 'java'; id 'org.springframework.boot' version '4.1.1' }\nrepositories { mavenCentral() }\ndependencies { implementation 'org.springframework.boot:spring-boot-starter:4.1.1' }\n"
        };
        std::fs::write(root.join(marker), manifest).unwrap();
        std::fs::write(
            root.join("settings.gradle"),
            "rootProject.name='boot-fixture'\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("src/main/java/test/anywhere")).unwrap();
        std::fs::write(root.join("src/main/java/test/anywhere/Fixture.java"), "package test.anywhere; import org.springframework.boot.*; import org.springframework.boot.autoconfigure.SpringBootApplication; @SpringBootApplication public class Fixture implements CommandLineRunner { public static void main(String[] args) { SpringApplication.run(Fixture.class,args); } public void run(String... args) throws Exception { System.out.println(\"BOOT-PID:\"+ProcessHandle.current().pid()); Thread.sleep(120000); } }\n").unwrap();
        let quoted = tool.to_string_lossy().replace('\'', "'\"'\"'");
        std::fs::write(
            root.join(wrapper),
            format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n"),
        )
        .unwrap();
        let app = app(Arc::new(TestPlatform::new(root, true)));
        let activation = app.activate().await.unwrap().unwrap();
        let state = settled(&app, activation.session_id, 0).await;
        let result = execute_action(&app, &state, command).await;
        assert!(result.succeeded(), "{:?}", result.error);
        let id = result.execution_session_id.unwrap();
        let pid = tokio::time::timeout(Duration::from_secs(180), async {
            loop {
                let task = app.execution_session(&id).unwrap();
                if let Some(pid) = task.stdout.lines().find_map(|line| {
                    line.strip_prefix("BOOT-PID:")
                        .and_then(|pid| pid.parse::<u32>().ok())
                }) {
                    break pid;
                }
                assert!(
                    task.summary.status.is_active(),
                    "{kind}: {}\n{}",
                    task.stdout,
                    task.stderr
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        app.stop_execution_session(&id).unwrap();
        assert_eq!(
            completed_task(&app, &id).await.summary.status,
            ExecutionSessionStatus::Stopped
        );
        let gone = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = std::process::Command::new("/bin/ps")
                    .args(["-o", "stat=", "-p", &pid.to_string()])
                    .output()
                    .unwrap();
                let state = String::from_utf8_lossy(&status.stdout);
                if !status.status.success()
                    || state.trim().starts_with('Z')
                    || state.trim().is_empty()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        let alive = gone.is_err();
        if alive {
            let process = std::process::Command::new("/bin/ps")
                .args(["-o", "pid,ppid,pgid,stat,args", "-p", &pid.to_string()])
                .output()
                .unwrap();
            println!(
                "Surviving process: {}",
                String::from_utf8_lossy(&process.stdout)
            );
            let _ = std::process::Command::new("/bin/kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
        assert!(!alive, "{kind} Spring Boot child survived Stop");
    }
}
