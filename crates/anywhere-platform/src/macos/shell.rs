//! Opening things: URLs, files, directories, a terminal, and Finder reveal.
use anyhow::{bail, Context, Result};
use std::{
    ffi::{OsStr, OsString},
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn open_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).context("invalid URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("only absolute http/https URLs are permitted");
    }
    run("/usr/bin/open", &[OsStr::new(url.as_str())], None).map(|_| ())
}

pub(super) fn open_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
}

pub(super) fn open_file_in_application(application: &Path, target: &Path) -> Result<()> {
    let application = application_path(application)?;
    anyhow::ensure!(
        target.is_absolute() && target.exists(),
        "history target unavailable"
    );
    let target = target.canonicalize()?;
    let bundle = super::application::bundle_application(&application)?;
    let args = file_open_arguments(
        &application,
        &target,
        bundle.as_ref().map(|app| app.bundle_id.as_str()),
    );
    run(
        "/usr/bin/open",
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        None,
    )
    .map(|_| ())
}

fn file_open_arguments(
    application: &Path,
    target: &Path,
    bundle_id: Option<&str>,
) -> Vec<OsString> {
    let jetbrains_ide = bundle_id.is_some_and(|id| id.starts_with("com.jetbrains."))
        && application
            .join("Contents/Resources/product-info.json")
            .is_file();
    if jetbrains_ide {
        // JetBrains' documented macOS launcher forwards arguments to its IDE
        // instance. A generic open-document event may not focus an open project.
        // Keep the explicit installation and target, with no title/name guessing.
        vec![
            "-n".into(),
            "-a".into(),
            application.into(),
            "--args".into(),
            target.into(),
        ]
    } else {
        vec!["-a".into(), application.into(), target.into()]
    }
}

pub(super) fn open_url_in_application(application: &Path, value: &str) -> Result<()> {
    let application = application_path(application)?;
    let url = url::Url::parse(value).context("invalid application URL")?;
    anyhow::ensure!(
        value.len() <= 16 * 1024
            && !value.contains('\0')
            && !matches!(url.scheme(), "file" | "javascript" | "data"),
        "unsupported application URL"
    );
    open_in_application(&application, OsStr::new(url.as_str()))
}

fn application_path(application: &Path) -> Result<std::path::PathBuf> {
    anyhow::ensure!(
        application.is_absolute()
            && application.is_dir()
            && application.extension().is_some_and(|ext| ext == "app"),
        "source application unavailable"
    );
    Ok(application.canonicalize()?)
}

fn open_in_application(application: &Path, target: &OsStr) -> Result<()> {
    run(
        "/usr/bin/open",
        &[OsStr::new("-a"), application.as_os_str(), target],
        None,
    )
    .map(|_| ())
}

pub(super) fn reveal_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[OsStr::new("-R"), path.as_os_str()], None).map(|_| ())
}

pub(super) fn open_directory(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
}

pub(super) fn open_terminal(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    // Terminal natively accepts directory arguments; no shell interpolation.
    run(
        "/usr/bin/open",
        &[OsStr::new("-a"), OsStr::new("Terminal"), path.as_os_str()],
        None,
    )
    .map(|_| ())
}

pub(super) fn require_directory(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("directory no longer exists: {}", path.display());
    }
    Ok(())
}

pub(super) fn run(program: &str, args: &[&OsStr], stdin: Option<&str>) -> Result<String> {
    let start = Instant::now();
    let result = (|| -> Result<String> {
        let mut child = Command::new(program)
            .args(args)
            // GUI launches often inherit the C locale. pbcopy silently clears
            // non-ASCII text under that locale, despite exiting successfully.
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("start {program}"))?;
        if let Some(text) = stdin {
            let mut pipe = child.stdin.take().context("child stdin missing")?;
            if let Err(error) = pipe.write_all(text.as_bytes()) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("write clipboard input");
            }
        }
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed() >= Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{program} timed out; check macOS Automation permission for Finder");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            bail!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout).context("platform response is not UTF-8")
    })();
    if result.is_err() {
        // System diagnostics may contain private history URLs or target paths.
        tracing::error!(program, "platform_api_failure");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::MacOsPlatform;
    use crate::Platform;

    #[test]
    fn jetbrains_projects_use_the_selected_installations_cli_with_literal_paths() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("IDE 中文 with spaces.app");
        let resources = app.join("Contents/Resources");
        std::fs::create_dir_all(&resources).unwrap();
        std::fs::write(resources.join("product-info.json"), "{}").unwrap();
        let target = root.path().join("项目 'quoted' [main] $(literal)");
        for id in [
            "com.jetbrains.intellij",
            "com.jetbrains.intellij.ce",
            "com.jetbrains.WebStorm-EAP",
        ] {
            assert_eq!(
                file_open_arguments(&app, &target, Some(id)),
                vec![
                    OsString::from("-n"),
                    "-a".into(),
                    app.clone().into_os_string(),
                    "--args".into(),
                    target.clone().into_os_string()
                ]
            );
        }
        // A renamed non-JetBrains app or an unrelated JetBrains utility must
        // keep the standard open-document route. Neither path may use a shell.
        for id in [None, Some("com.microsoft.VSCode")] {
            assert_eq!(
                file_open_arguments(&app, &target, id),
                vec![
                    OsString::from("-a"),
                    app.clone().into_os_string(),
                    target.clone().into_os_string()
                ]
            );
        }
        std::fs::remove_file(resources.join("product-info.json")).unwrap();
        assert_eq!(
            file_open_arguments(&app, &target, Some("com.jetbrains.toolbox")),
            vec![
                OsString::from("-a"),
                app.into_os_string(),
                target.into_os_string()
            ]
        );
    }

    #[test]
    fn platform_rejects_unsafe_urls_and_missing_paths_without_side_effects() {
        let platform = MacOsPlatform;
        assert!(platform.open_url("javascript:alert(1)").is_err());
        assert!(platform.open_url("file:///etc/passwd").is_err());
        assert!(platform
            .open_file(Path::new("/nonexistent-launcher-test/file"))
            .is_err());
        let temporary = tempfile::tempdir().unwrap();
        let app = temporary.path().join("Test.app");
        std::fs::create_dir(&app).unwrap();
        for url in [
            "javascript:alert(1)",
            "data:text/plain,test",
            "file:///tmp/test",
            "relative",
        ] {
            assert!(platform.open_url_in_application(&app, url).is_err());
        }
        assert!(platform
            .open_directory(Path::new("/nonexistent-launcher-test"))
            .is_err());
        assert!(platform
            .open_file_in_application(
                Path::new("/nonexistent-launcher-test.app"),
                Path::new("/tmp")
            )
            .is_err());
    }
}
