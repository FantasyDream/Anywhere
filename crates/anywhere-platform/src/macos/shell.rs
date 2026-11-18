//! Opening things: URLs, files, directories, a terminal, and Finder reveal.
use anyhow::{bail, Context, Result};
use std::{
    ffi::{CString, OsStr, OsString},
    path::Path,
};

fn workspace_open(
    target: &OsStr,
    file: bool,
    application: Option<&Path>,
    operation: i32,
) -> Result<()> {
    let target = CString::new(target.to_str().context("target is not UTF-8")?)?;
    let application = application
        .map(|path| {
            CString::new(path.to_str().context("application path is not UTF-8")?)
                .map_err(anyhow::Error::from)
        })
        .transpose()?;
    // SAFETY: C strings live for this bounded synchronous call; Cocoa copies
    // target/application URLs before scheduling its completion handler.
    let status = unsafe {
        super::native::launcher_open_resource(
            target.as_ptr(),
            i32::from(file),
            application
                .as_ref()
                .map_or(std::ptr::null(), |path| path.as_ptr()),
            operation,
        )
    };
    anyhow::ensure!(status == 1, "native resource open failed");
    Ok(())
}

pub(super) fn open_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).context("invalid URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("only absolute http/https URLs are permitted");
    }
    workspace_open(OsStr::new(url.as_str()), false, None, 0)
}

pub(super) fn open_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    workspace_open(path.as_os_str(), true, None, 0)
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
    let operation = if args.first().is_some_and(|arg| arg == "-n") {
        3
    } else {
        0
    };
    workspace_open(target.as_os_str(), true, Some(&application), operation)
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
    workspace_open(target, false, Some(application), 0)
}

pub(super) fn reveal_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    workspace_open(path.as_os_str(), true, None, 1)
}

pub(super) fn open_directory(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    workspace_open(path.as_os_str(), true, None, 0)
}

pub(super) fn open_terminal(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    workspace_open(path.as_os_str(), true, None, 2)
}

pub(super) fn require_directory(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("directory no longer exists: {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::MacOsPlatform;
    use crate::ResourceOpener;

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
