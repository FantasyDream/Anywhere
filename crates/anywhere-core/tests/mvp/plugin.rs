use crate::support::*;

fn registry_with_wrap() -> ActionRegistry {
    let mut registry = registry();
    registry
        .register_plugin(anywhere_plugin::parse_manifest(crate::search::WRAP_PLUGIN).unwrap())
        .unwrap();
    registry
}

#[tokio::test]
async fn wrapping_a_git_action_is_disabled_when_git_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let platform = Arc::new(TestPlatform {
        no_git: true,
        ..TestPlatform::new(root, true)
    });
    let app = Application::new(
        platform.clone(),
        registry_with_wrap(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    for command in ["core.git.status", "wrap.status"] {
        let status = state
            .candidates
            .iter()
            .find(|candidate| candidate.command_id.as_deref() == Some(command))
            .unwrap_or_else(|| panic!("missing {command}"));
        assert_eq!(status.availability.state, AvailabilityState::Disabled);
        let error = app
            .execute(state.session_id, state.revision, status.id.clone())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Git 不可用"),
            "{command}: {error}"
        );
    }
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn wrapping_copy_path_is_hidden_when_the_clipboard_already_holds_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let platform = Arc::new(TestPlatform {
        clipboard: Mutex::new(Some(root.to_string_lossy().into_owned())),
        ..TestPlatform::new(root, true)
    });
    let app = Application::new(
        platform,
        registry_with_wrap(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let state = settled(&app, app.activate().await.unwrap().unwrap().session_id, 0).await;
    let visible = state
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CandidateKind::Action)
        .filter_map(|candidate| candidate.command_id.as_deref())
        .collect::<Vec<_>>();
    assert!(!visible.contains(&"core.directory.copy"), "{visible:?}");
    assert!(!visible.contains(&"wrap.copy"), "{visible:?}");
    let searched = app
        .update_input(state.session_id, "copy".into(), 1)
        .await
        .unwrap()
        .unwrap();
    let searched = searched
        .candidates
        .iter()
        .filter_map(|candidate| candidate.command_id.as_deref())
        .collect::<Vec<_>>();
    assert!(!searched.contains(&"core.directory.copy"), "{searched:?}");
    assert!(!searched.contains(&"wrap.copy"), "{searched:?}");
}

#[tokio::test]
async fn wrapping_json_keeps_plugin_identity_and_records_usage() {
    let history = Arc::new(Mutex::new(MemoryUsage::default()));
    let app = Application::new(
        Arc::new(TestPlatform::default()),
        registry_with_wrap(),
        Box::new(SharedHistory(history.clone())),
        vec![],
    );
    let session = app.activate().await.unwrap().unwrap().session_id;
    let json = app
        .update_input(session, r#"{"a":1}"#.into(), 1)
        .await
        .unwrap()
        .unwrap();
    let pretty = json
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some("wrap.pretty"))
        .expect("wrapped json action");
    assert_eq!(pretty.title, "Pretty JSON");
    assert!(pretty
        .subtitle
        .as_deref()
        .is_some_and(|subtitle| subtitle.contains("Wrap")));
    assert!(app
        .execute(session, json.revision, pretty.id.clone())
        .await
        .unwrap()
        .succeeded());
    let recorded = history
        .lock()
        .unwrap()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        recorded
            .iter()
            .any(|(action, kind, _)| { action == "wrap.pretty" && kind == JSON_DOCUMENT }),
        "{recorded:?}"
    );
}

#[test]
fn invalid_plugin_does_not_partially_register_and_discovery_continues() {
    let mut registry = registry();
    let mut plugin = anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap();
    plugin.id = "broken".into();
    let mut bad = plugin.commands[0].clone();
    bad.id = "bad".into();
    bad.action = DeclarativeAction::ExecuteBuiltin {
        command_id: "core.json.format".into(),
    };
    plugin.commands.push(bad);
    assert!(registry.register_plugin(plugin).is_err());
    // Nothing from the rejected manifest may be registered, under any id.
    assert!(registry.get("broken.search").is_none());
    assert!(registry.get("broken.bad").is_none());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("good.toml"),
        anywhere_plugin::EXAMPLE_PLUGIN,
    )
    .unwrap();
    std::fs::write(dir.path().join("bad.toml"), "this is not toml").unwrap();
    let discovered = anywhere_plugin::discover(dir.path());
    assert_eq!(discovered.manifests.len(), 1);
    assert_eq!(discovered.diagnostics.len(), 1);
    assert!(discovered.diagnostics[0].contains("bad.toml"));
}
