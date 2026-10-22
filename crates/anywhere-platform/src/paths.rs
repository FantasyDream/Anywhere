//! Home-directory resolution shared by context recognition and file search.
//! This is lexical expansion only: never invoke a shell or expand variables.
use std::path::{Path, PathBuf};

pub fn home_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    let keys = ["USERPROFILE", "HOME"];
    #[cfg(not(windows))]
    let keys = ["HOME", "USERPROFILE"];
    keys.into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
}

/// Expand `~`, `~/…`, and the full-width `～` spelling at the start only.
/// Other characters, spaces, and relative paths keep their literal meaning.
pub fn expand_home(raw: &str) -> Option<PathBuf> {
    expand_home_with(raw, home_directory().as_deref())
}

fn expand_home_with(raw: &str, home: Option<&Path>) -> Option<PathBuf> {
    let Some(tail) = raw.strip_prefix('~').or_else(|| raw.strip_prefix('～')) else {
        return Some(PathBuf::from(raw));
    };
    if !tail.is_empty() && !tail.starts_with(std::path::is_separator) {
        return Some(PathBuf::from(raw));
    }
    let home = home.filter(|path| path.is_absolute())?;
    // Path::join replaces its base for an absolute tail. Repeated separators
    // must remain inside Home instead of turning `~//etc` into `/etc`.
    Some(home.join(tail.trim_start_matches(std::path::is_separator)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_both_home_spellings_without_changing_the_filename() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        for prefix in ["~", "～"] {
            assert_eq!(expand_home_with(prefix, Some(home)), Some(home.into()));
            for suffix in [
                "/",
                "/Documents/项目 with spaces.txt",
                "/trailing ",
                "//etc",
            ] {
                assert_eq!(
                    expand_home_with(&format!("{prefix}{suffix}"), Some(home)),
                    Some(home.join(suffix.trim_start_matches('/')))
                );
            }
        }
    }

    #[test]
    fn missing_home_never_falls_back_to_the_working_directory() {
        for input in ["~", "~/file", "～", "～/file"] {
            assert!(expand_home_with(input, None).is_none());
            assert!(expand_home_with(input, Some(Path::new("relative"))).is_none());
        }
    }

    #[test]
    fn other_paths_and_shell_expressions_are_literal() {
        for input in [
            "~other/file",
            "～note.txt",
            "$HOME/file",
            "./file",
            "folder/～.txt",
        ] {
            assert_eq!(expand_home_with(input, None), Some(input.into()));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_separators_also_stay_under_home() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            expand_home_with(r"~\\Documents\file.txt", Some(directory.path())),
            Some(directory.path().join(r"Documents\file.txt"))
        );
    }
}
