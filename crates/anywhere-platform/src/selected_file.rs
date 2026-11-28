//! Native identity for a single regular file captured from the OS selection.
use anyhow::{ensure, Result};
use std::{fs::File, path::Path};

/// A platform file identity captured before publishing a Finder or Explorer selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedFileIdentity {
    volume: u64,
    object: [u8; 16],
}

impl SelectedFileIdentity {
    /// Read a stable identity from a regular, locally available selected file.
    ///
    /// # Errors
    /// Returns an error if the path cannot be safely opened or lacks a stable identity.
    pub fn capture(path: &Path) -> Result<Self> {
        native::capture(path)
    }

    /// Obtain a fresh read-only descriptor for the same captured object.
    ///
    /// # Errors
    /// Returns an error if an ancestor or the file has been replaced or is unsafe to open.
    pub fn open(self, path: &Path) -> Result<File> {
        let (file, identity) = native::open(path)?;
        ensure!(self == identity, "selected file identity changed");
        Ok(file)
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use anyhow::{bail, Context};
    use std::{
        ffi::CString,
        io,
        mem::MaybeUninit,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
        path::Component,
    };

    const SF_DATALESS: u32 = 0x4000_0000;

    fn fstat_at(parent: libc::c_int, name: &std::ffi::CStr) -> io::Result<libc::stat> {
        let mut stat = MaybeUninit::<libc::stat>::uninit();
        // SAFETY: The C string is alive, parent is an owned dir FD, and fstatat initializes stat.
        if unsafe {
            libc::fstatat(
                parent,
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fstatat succeeded.
        Ok(unsafe { stat.assume_init() })
    }

    fn fstat_fd(fd: libc::c_int) -> io::Result<libc::stat> {
        let mut stat = MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstat writes the complete stat structure on success.
        if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fstat succeeded.
        Ok(unsafe { stat.assume_init() })
    }

    fn check_local_fs(fd: libc::c_int) -> Result<()> {
        let mut filesystem = MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: fstatfs writes the complete filesystem structure on success.
        if unsafe { libc::fstatfs(fd, filesystem.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: fstatfs succeeded.
        let filesystem = unsafe { filesystem.assume_init() };
        ensure!(
            filesystem.f_flags & libc::MNT_LOCAL.cast_unsigned() != 0,
            "selected file is on a remote volume"
        );
        Ok(())
    }

    fn open_at(parent: libc::c_int, name: &std::ffi::CStr, kind: libc::c_int) -> io::Result<File> {
        // SAFETY: The C string is alive and parent is an owned directory FD or AT_FDCWD.
        let raw = unsafe {
            libc::openat(
                parent,
                name.as_ptr(),
                kind | libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(raw) })
    }

    fn identity_from_stat(stat: &libc::stat) -> SelectedFileIdentity {
        let mut object = [0; 16];
        object[..8].copy_from_slice(&stat.st_ino.to_le_bytes());
        SelectedFileIdentity {
            volume: stat.st_dev.cast_unsigned().into(),
            object,
        }
    }

    fn walk_to_parent(path: &Path) -> Result<(File, CString, libc::stat)> {
        let mut components = path.components().peekable();
        ensure!(
            components.next() == Some(Component::RootDir),
            "selected file requires an absolute path"
        );
        let mut parent = open_at(libc::AT_FDCWD, c"/", libc::O_DIRECTORY)?;
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                bail!("invalid selected file path")
            };
            let name = CString::new(name.as_bytes()).context("invalid selected file name")?;
            let pre = fstat_at(parent.as_raw_fd(), &name)?;
            ensure!(
                pre.st_flags & SF_DATALESS == 0,
                "selected file is a cloud placeholder"
            );
            if components.peek().is_some() {
                ensure!(
                    pre.st_mode & libc::S_IFMT == libc::S_IFDIR,
                    "selected file has a redirected ancestor"
                );
                let next = open_at(parent.as_raw_fd(), &name, libc::O_DIRECTORY)?;
                let opened = fstat_fd(next.as_raw_fd())?;
                ensure!(
                    opened.st_dev == pre.st_dev
                        && opened.st_ino == pre.st_ino
                        && opened.st_flags & SF_DATALESS == 0,
                    "selected file ancestor changed"
                );
                parent = next;
                continue;
            }
            // Check local mount and regular, non-dataless metadata BEFORE any O_RDONLY open
            // so evicted iCloud Drive / FileProvider placeholders never trigger downloads.
            check_local_fs(parent.as_raw_fd())?;
            ensure!(
                pre.st_mode & libc::S_IFMT == libc::S_IFREG,
                "selected object is not a regular file"
            );
            return Ok((parent, name, pre));
        }
        bail!("selected path is a directory");
    }

    pub(super) fn capture(path: &Path) -> Result<SelectedFileIdentity> {
        let (_parent, _name, pre) = walk_to_parent(path)?;
        Ok(identity_from_stat(&pre))
    }

    pub(super) fn open(path: &Path) -> Result<(File, SelectedFileIdentity)> {
        let (parent, name, pre) = walk_to_parent(path)?;
        let file = open_at(parent.as_raw_fd(), &name, 0)?;
        let stat = fstat_fd(file.as_raw_fd())?;
        ensure!(
            stat.st_mode & libc::S_IFMT == libc::S_IFREG
                && stat.st_flags & SF_DATALESS == 0
                && stat.st_dev == pre.st_dev
                && stat.st_ino == pre.st_ino,
            "selected file changed or is unavailable"
        );
        check_local_fs(file.as_raw_fd())?;
        Ok((file, identity_from_stat(&stat)))
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        ffi::OsString,
        fs::OpenOptions,
        os::windows::{
            ffi::OsStringExt,
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
        path::{Component, PathBuf, Prefix},
    };
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FileAttributeTagInfo, FileIdInfo, GetFileInformationByHandleEx,
            GetFinalPathNameByHandleW, GetVolumeInformationByHandleW, FILE_ATTRIBUTE_OFFLINE,
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_NAME_NORMALIZED, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE,
        },
    };

    const CLOUD_TAG: u32 = 0x9000_001a;
    const CLOUD_VARIANTS: u32 = 0x0000_f000;
    const FILE_SUPPORTS_OPEN_BY_FILE_ID: u32 = 0x0100_0000;
    const UNAVAILABLE: u32 = FILE_ATTRIBUTE_OFFLINE.0
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0
        | FILE_ATTRIBUTE_RECALL_ON_OPEN.0;

    fn is_cloud_reparse_tag(tag: u32) -> bool {
        tag & !CLOUD_VARIANTS == CLOUD_TAG
    }

    fn tag_info(file: &File) -> Result<FILE_ATTRIBUTE_TAG_INFO> {
        let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
        // SAFETY: The handle outlives this call and the buffer is writable with the native size.
        unsafe {
            GetFileInformationByHandleEx(
                HANDLE(file.as_raw_handle()),
                FileAttributeTagInfo,
                (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )?;
        }
        Ok(info)
    }

    fn open_dir(path: &Path) -> Result<File> {
        Ok(OpenOptions::new()
            .access_mode(0)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
            .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
            .open(path)?)
    }

    fn ancestors(path: &Path) -> Result<()> {
        let drive = path.components().next().and_then(|component| {
            let Component::Prefix(prefix) = component else {
                return None;
            };
            match prefix.kind() {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => Some(drive),
                _ => None,
            }
        });
        let drive = drive
            .filter(|_| {
                path.is_absolute()
                    && path
                        .components()
                        .all(|part| !matches!(part, Component::ParentDir))
            })
            .ok_or_else(|| anyhow::anyhow!("selected file requires a local absolute path"))?;
        let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
        // SAFETY: the local drive root is a NUL-terminated UTF-16 string.
        let drive_type = unsafe {
            windows::Win32::Storage::FileSystem::GetDriveTypeW(windows::core::PCWSTR(root.as_ptr()))
        };
        ensure!(
            matches!(drive_type, 2 | 3),
            "selected file is not on a local drive"
        );
        for ancestor in path.ancestors().skip(1) {
            if !ancestor.has_root() {
                break;
            }
            let metadata = ancestor.symlink_metadata()?;
            ensure!(metadata.is_dir(), "selected file has a redirected ancestor");
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                let dir = open_dir(ancestor)?;
                let tag = tag_info(&dir)?;
                ensure!(
                    tag.FileAttributes & UNAVAILABLE == 0 && is_cloud_reparse_tag(tag.ReparseTag),
                    "selected file has a redirected ancestor"
                );
            }
        }
        Ok(())
    }

    fn open_path(path: &Path, flags: u32) -> Result<File> {
        Ok(OpenOptions::new()
            .read(true)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
            .custom_flags(flags)
            .open(path)?)
    }

    fn final_path(file: &File) -> Result<PathBuf> {
        let mut buffer = vec![0u16; 512];
        loop {
            // SAFETY: Handle is live and buffer is writable for its reported length.
            let len = unsafe {
                GetFinalPathNameByHandleW(
                    HANDLE(file.as_raw_handle()),
                    &mut buffer,
                    FILE_NAME_NORMALIZED,
                )
            } as usize;
            ensure!(len > 0, "selected file final path unavailable");
            if len < buffer.len() {
                let path = PathBuf::from(OsString::from_wide(&buffer[..len]));
                return Ok(dunce::simplified(&path).to_path_buf());
            }
            ensure!(len <= 32_768, "selected file final path too long");
            buffer.resize(len + 1, 0);
        }
    }

    fn identity(file: &File) -> Result<SelectedFileIdentity> {
        let mut fs_flags = 0u32;
        // SAFETY: Handle is live; optional output pointers are null.
        unsafe {
            GetVolumeInformationByHandleW(
                HANDLE(file.as_raw_handle()),
                None,
                None,
                None,
                Some(&mut fs_flags),
                None,
            )?;
        }
        ensure!(
            fs_flags & FILE_SUPPORTS_OPEN_BY_FILE_ID != 0,
            "selected file volume lacks stable file identifiers"
        );
        let mut info = FILE_ID_INFO::default();
        // SAFETY: The live file owns the HANDLE and the information buffer has the exact native shape.
        unsafe {
            GetFileInformationByHandleEx(
                HANDLE(file.as_raw_handle()),
                FileIdInfo,
                (&mut info as *mut FILE_ID_INFO).cast(),
                size_of::<FILE_ID_INFO>() as u32,
            )?;
        }
        ensure!(
            info.VolumeSerialNumber != 0 && info.FileId.Identifier != [0; 16],
            "selected file has no stable identifier"
        );
        Ok(SelectedFileIdentity {
            volume: info.VolumeSerialNumber,
            object: info.FileId.Identifier,
        })
    }

    fn attributes(file: &File) -> Result<FILE_ATTRIBUTE_TAG_INFO> {
        let info = tag_info(file)?;
        ensure!(
            file.metadata()?.is_file() && info.FileAttributes & UNAVAILABLE == 0,
            "selected object is unavailable or not a regular file"
        );
        Ok(info)
    }

    pub(super) fn capture(path: &Path) -> Result<SelectedFileIdentity> {
        let (_, id) = open(path)?;
        Ok(id)
    }

    pub(super) fn open(path: &Path) -> Result<(File, SelectedFileIdentity)> {
        ancestors(path)?;
        let expected_parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("selected file has no parent directory"))?;
        let expected_parent_id = identity(&open_dir(expected_parent)?)?;
        let file = open_path(path, FILE_FLAG_OPEN_REPARSE_POINT.0)?;
        let tag = attributes(&file)?;
        let redirected = tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0;
        ensure!(
            !redirected || is_cloud_reparse_tag(tag.ReparseTag),
            "selected file is redirected"
        );
        let id = identity(&file)?;
        let file = if redirected {
            // Hydrated Cloud Files reparse records describe this same file, including its ID.
            let resolved = open_path(path, 0)?;
            let _ = attributes(&resolved)?;
            ensure!(id == identity(&resolved)?, "selected cloud file changed");
            resolved
        } else {
            file
        };
        let resolved_path = final_path(&file)?;
        ancestors(&resolved_path)?;
        let resolved_parent = resolved_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("selected file final path has no parent"))?;
        ensure!(
            identity(&open_dir(resolved_parent)?)? == expected_parent_id,
            "selected file ancestor changed during open"
        );
        ensure!(
            resolved_path
                .file_name()
                .zip(path.file_name())
                .is_some_and(|(a, b)| a
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&b.to_string_lossy())),
            "selected file name changed during open"
        );
        ancestors(path)?;
        Ok((file, id))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;

    pub(super) fn capture(_: &Path) -> Result<SelectedFileIdentity> {
        anyhow::bail!("native selected file identity unavailable");
    }

    pub(super) fn open(_: &Path) -> Result<(File, SelectedFileIdentity)> {
        anyhow::bail!("native selected file identity unavailable");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_and_open_verify_regular_file_and_detect_replacement_or_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sample.csv");
        std::fs::write(&file, "a,b\n1,2\n").unwrap();

        let id = SelectedFileIdentity::capture(&file).unwrap();
        assert!(id.open(&file).is_ok());
        assert!(SelectedFileIdentity::capture(dir.path()).is_err());
        assert!(SelectedFileIdentity::capture(Path::new("relative.csv")).is_err());

        let replacement = dir.path().join("replacement.csv");
        std::fs::write(&replacement, "x,y\n").unwrap();
        std::fs::remove_file(&file).unwrap();
        std::fs::rename(&replacement, &file).unwrap();
        assert!(id.open(&file).is_err());
    }
}
