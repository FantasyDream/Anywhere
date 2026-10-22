//! Preparing a private local data file is an OS concern, independent of SQLite.
use anyhow::{ensure, Result};
use std::{fs::OpenOptions, path::Path};

pub fn prepare_file(path: &Path) -> Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        ensure!(
            metadata.file_type().is_file(),
            "private storage requires a regular file"
        );
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "private storage requires a regular file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    #[test]
    fn existing_and_new_files_are_private_and_symlinks_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("history");
        prepare_file(&file).unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        prepare_file(&file).unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let alias = directory.path().join("alias");
        symlink(&file, &alias).unwrap();
        assert!(prepare_file(&alias).is_err());
    }
}
