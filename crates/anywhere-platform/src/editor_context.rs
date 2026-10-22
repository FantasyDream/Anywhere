//! Conservative interpretation of explicit editor project metadata, independent of OS APIs.
use std::path::{Path, PathBuf};

pub(crate) fn supports_window_project(bundle_id: &str) -> bool {
    matches!(
        bundle_id,
        "com.jetbrains.intellij" | "com.jetbrains.intellij.ce" | "com.jetbrains.intellij-EAP"
    )
}

/// IDEA's project window title: `project [~/project/path] – opened file`.
/// Never infer a project from a filename, project name, history, or source text.
pub(crate) fn idea_project_directory(title: &str, home: Option<&Path>) -> Option<PathBuf> {
    if title.len() > 8192 || title.contains(['\n', '\r']) {
        return None;
    }
    let prefix = title
        .split_once("] – ")
        .map(|(prefix, _)| prefix)
        .or_else(|| title.strip_suffix(']'))?;
    let (name, path) = prefix.split_once(" [")?;
    if name.is_empty() || path.is_empty() {
        return None;
    }
    let path = if let Some(relative) = path.strip_prefix("~/") {
        let home = home.filter(|home| home.is_absolute())?;
        home.join(relative)
    } else {
        PathBuf::from(path)
    };
    if !path.is_absolute() || !path.is_dir() || !path.join(".idea").is_dir() {
        return None;
    }
    path.canonicalize().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_idea_project_paths_preserve_spaces_unicode_and_brackets() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("项目 [main] with spaces");
        std::fs::create_dir_all(project.join(".idea")).unwrap();
        let expected = project.canonicalize().unwrap();
        assert_eq!(
            idea_project_directory(
                &format!("project [{}] – /other/File.java", project.display()),
                None
            ),
            Some(expected.clone())
        );
        assert_eq!(
            idea_project_directory(
                "project [~/项目 [main] with spaces] – Foo.java",
                Some(home.path())
            ),
            Some(expected.clone())
        );
        assert_eq!(
            idea_project_directory(&format!("project [{}]", project.display()), None),
            Some(expected)
        );
    }
    #[test]
    fn missing_or_ambiguous_project_metadata_does_not_guess_a_directory() {
        let temp = tempfile::tempdir().unwrap();
        for title in [
            "project – Foo.java",
            "project [relative/path] – Foo.java",
            "project [~/missing] – Foo.java",
            "project [/missing] – Foo.java",
            "project [~/repo]\n",
        ] {
            assert!(idea_project_directory(title, Some(temp.path())).is_none());
        }
        assert!(idea_project_directory(
            &format!("project [{}] – Foo.java", temp.path().display()),
            None
        )
        .is_none());
        assert!(idea_project_directory("project [~/repo] – Foo.java", None).is_none());
        assert!(!supports_window_project("com.microsoft.VSCode"));
        assert!(!supports_window_project("com.openai.codex"));
    }
}
