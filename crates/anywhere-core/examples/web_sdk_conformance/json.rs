//! Real JSON bundles with synthetic storage/clipboard, including grant checks and headless routing.
use super::*;
use anywhere_core::application::SnapshotWait;
use anywhere_protocol::{web_plugin::WebInput, InputMode, LauncherState, SessionId};
use std::time::Duration;

const PREFIX: &str = "web:dev.anywhere.json-workspace:";

async fn query(
    app: &Application,
    id: SessionId,
    revision: u32,
    text: &str,
    mode: InputMode,
) -> Result<LauncherState> {
    let mut state = app
        .update_context_input(id, text.into(), revision, None, mode)
        .await?
        .context("query did not publish")?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !state.context_complete {
            let cursor = (u64::from(state.revision) << 32) | u64::from(state.snapshot_seq);
            let SnapshotWait::Updated(next) = app.wait_for_snapshot(id, cursor).await else {
                anyhow::bail!("query ended before Context completed")
            };
            state = *next;
        }
        Ok::<_, anyhow::Error>(state)
    })
    .await?
}

fn commands(state: &LauncherState) -> Vec<&str> {
    state
        .candidates
        .iter()
        .filter_map(|candidate| candidate.command_id.as_deref())
        .filter_map(|id| id.strip_prefix(PREFIX))
        .collect()
}

async fn open(
    app: &Application,
    state: &LauncherState,
    command: &str,
) -> Result<WebDocumentBinding> {
    let candidate = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(&format!("{PREFIX}{command}")))
        .with_context(|| format!("missing command: {command}"))?;
    let result = app
        .execute(state.session_id, state.revision, candidate.id.clone())
        .await?;
    let Some(HostView::WebPlugin { invocation_id }) = result.view else {
        anyhow::bail!("command failed: {:?}", result.error)
    };
    app.bind_web_document(&invocation_id, WebPlatform::Macos)
}

async fn copy(app: &Application, state: &LauncherState, command: &str) -> Result<()> {
    let candidate = state
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(&format!("{PREFIX}{command}")))
        .context("copy command missing")?;
    let result = app
        .execute(state.session_id, state.revision, candidate.id.clone())
        .await?;
    anyhow::ensure!(
        result.succeeded() && result.view.is_none() && result.close_window,
        "copy must finish without a tool page: {:?}",
        result.error
    );
    Ok(())
}

pub(super) async fn check(path: &Path) -> Result<()> {
    let platform = Arc::new(SyntheticPlatform(Mutex::new("untouched".into())));
    let app = Arc::new(Application::new(
        platform.clone(),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    ));
    app.configure_web_storage(ModuleStorageHost::memory()?)?;
    let root = tempfile::tempdir()?;
    let mut store = WebPackageStore::open(root.path().join("plugins"), WebPlatform::Macos, &app)?;
    let review = store.inspect(path)?;
    store.approve(&app, &review.token)?;
    let id = app.begin_activation()?;
    let keyword = query(&app, id, 1, "JSON", InputMode::Search).await?;
    anyhow::ensure!(
        commands(&keyword) == ["open"],
        "unusable helpers leaked into keyword search"
    );
    // Saved UI preferences must also be readable through the background adapter.
    let workspace = open(&app, &keyword, "open").await?;
    anyhow::ensure!(
        !app.web_document_is_background(&workspace)?,
        "workspace lost its UI"
    );
    let frame = json!({"protocol":1,"requestId":"indent","method":"storage.set",
        "params":{"key":"indent","value":4}})
    .to_string();
    let reply = app
        .dispatch_web_message(&workspace, source(&workspace), &frame)
        .await;
    let reply: Value = serde_json::from_str(&reply)?;
    anyhow::ensure!(
        reply.get("error").is_none(),
        "could not save indent: {reply}"
    );
    app.close_web_document(&workspace)?;
    let raw = r#"{"n":900719925474099312345,"n":1e9999,"text":"中文\n"}"#;
    let captured = anywhere_protocol::ContextCapturePresentation {
        source: anywhere_protocol::ContextSource::Clipboard,
        status: anywhere_protocol::ContextCaptureStatus::Captured,
        captured_at_ms: 1000,
        clipboard_age_ms: Some(0),
        text: Some(raw.into()),
    };
    let activation = app
        .choose_activation_input(std::slice::from_ref(&captured), 1000, false, true, false)
        .context("hybrid resolver did not recognize fresh clipboard JSON")?;
    anyhow::ensure!(activation.text == raw, "activation replaced original JSON");
    anyhow::ensure!(
        app.choose_activation_input(&[captured], 16001, false, true, false)
            .is_none(),
        "hybrid recognition bypassed clipboard expiry"
    );
    let plain = query(&app, id, 2, raw, InputMode::Content).await?;
    anyhow::ensure!(
        commands(&plain).len() == 4 && !commands(&plain).contains(&"unescape"),
        "plain JSON did not resolve the expected commands: {:?}",
        commands(&plain)
    );
    anyhow::ensure!(
        commands(&plain).first() == Some(&"open"),
        "fresh JSON must keep the declared workspace entry ahead of unused copy helpers"
    );
    let workspace = open(&app, &plain, "open").await?;
    anyhow::ensure!(
        matches!(app.web_invocation(&workspace, source(&workspace))?.input,
        WebInput::Data { ref type_id, ref value } if type_id == "plugin.dev.anywhere.json-workspace.json" && value["text"] == raw),
        "workspace did not receive the original typed Context"
    );
    app.close_web_document(&workspace)?;
    for (index, command) in ["copy-format", "copy-minify", "copy-escaped"]
        .into_iter()
        .enumerate()
    {
        let state = query(&app, id, index as u32 + 3, raw, InputMode::Content).await?;
        copy(&app, &state, command).await?;
        let actual = platform.0.lock().unwrap().clone();
        match command {
            "copy-format" => anyhow::ensure!(actual == "{\n    \"n\": 900719925474099312345,\n    \"n\": 1e9999,\n    \"text\": \"中文\\n\"\n}", "formatted JSON or saved indent changed: {actual}"),
            "copy-minify" => anyhow::ensure!(actual == raw, "minifying changed JSON tokens"),
            _ => anyhow::ensure!(serde_json::from_str::<String>(&actual)? == raw, "escaping changed JSON tokens"),
        }
    }
    let encoded = serde_json::to_string(raw)?;
    let nested = serde_json::to_string(&encoded)?;
    let mut revision = 6;
    for text in [&encoded[..], &encoded[1..encoded.len() - 1], &nested[..]] {
        let escaped = query(&app, id, revision, text, InputMode::Content).await?;
        revision += 1;
        let mut commands = commands(&escaped);
        commands.sort_unstable();
        anyhow::ensure!(
            commands == ["copy-escaped", "copy-format", "copy-minify", "open"],
            "encoded JSON must resolve the same four actions: {commands:?}"
        );
        let binding = open(&app, &escaped, "open").await?;
        let invocation = app.web_invocation(&binding, source(&binding))?;
        anyhow::ensure!(
            invocation.entrypoint == "workspace"
                && matches!(invocation.input,
            WebInput::Data { ref type_id, ref value } if type_id == "plugin.dev.anywhere.json-workspace.json" && value["text"] == raw),
            "JSON Context did not normalize outer escaping"
        );
        app.close_web_document(&binding)?;
        let state = query(&app, id, revision, text, InputMode::Content).await?;
        revision += 1;
        copy(&app, &state, "copy-minify").await?;
        anyhow::ensure!(
            *platform.0.lock().unwrap() == raw,
            "normalized JSON tokens changed during copying"
        );
    }
    for (index, invalid) in ["123", "true", "null", "ordinary text", "{broken}", "[1,]"]
        .into_iter()
        .enumerate()
    {
        let state = query(
            &app,
            id,
            index as u32 + revision,
            invalid,
            InputMode::Content,
        )
        .await?;
        anyhow::ensure!(
            commands(&state).is_empty(),
            "invalid JSON produced a helper: {invalid}"
        );
    }
    println!("JSON hybrid package conformance passed (resolver, UI routes, headless copies, saved indent, exact tokens, installed grants, no UI for copies; synthetic clipboard only)");
    Ok(())
}
