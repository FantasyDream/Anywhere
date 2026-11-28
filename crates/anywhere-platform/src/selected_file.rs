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
        let (_, identity) = native::open(path)?;
        Ok(identity)
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

    pub(super) fn open(path: &Path) -> Result<(File, SelectedFileIdentity)> {
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
            if components.peek().is_some() {
                parent = open_at(parent.as_raw_fd(), &name, libc::O_DIRECTORY)?;
                continue;
            }
            let file = open_at(parent.as_raw_fd(), &name, 0)?;
            let mut stat = MaybeUninit::<libc::stat>::uninit();
            // SAFETY: fstat writes the complete stat structure on success.
            if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error().into());
            }
            // SAFETY: fstat succeeded.
            let stat = unsafe { stat.assume_init() };
            ensure!(
                stat.st_mode & libc::S_IFMT == libc::S_IFREG,
                "selected object is not a regular file"
            );
            ensure!(
                stat.st_flags & SF_DATALESS == 0,
                "selected file is a cloud placeholder"
            );
            let mut filesystem = MaybeUninit::<libc::statfs>::uninit();
            // SAFETY: fstatfs writes the complete filesystem structure on success.
            if unsafe { libc::fstatfs(file.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error().into());
            }
            // SAFETY: fstatfs succeeded.
            let filesystem = unsafe { filesystem.assume_init() };
            ensure!(
                filesystem.f_flags & libc::MNT_LOCAL.cast_unsigned() != 0,
                "selected file is on a remote volume"
            );
            let mut object = [0; 16];
            object[..8].copy_from_slice(&stat.st_ino.to_le_bytes());
            return Ok((
                file,
                SelectedFileIdentity {
                    volume: stat.st_dev.cast_unsigned().into(),
                    object,
                },
            ));
        }
        bail!("selected path is a directory");
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        fs::OpenOptions,
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
        path::{Component, Prefix},
    };
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FileAttributeTagInfo, FileIdInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_OFFLINE,
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_ID_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        },
    };

    const CLOUD_TAG: u32 = 0x9000_001a;
    const CLOUD_VARIANTS: u32 = 0x0000_f000;
    const UNAVAILABLE: u32 = FILE_ATTRIBUTE_OFFLINE.0
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0
        | FILE_ATTRIBUTE_RECALL_ON_OPEN.0;

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
            ensure!(
                metadata.is_dir()
                    && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0,
                "selected file has a redirected ancestor"
            );
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

    fn identity(file: &File) -> Result<SelectedFileIdentity> {
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
            info.FileId.Identifier != [0; 16],
            "selected file has no stable identifier"
        );
        Ok(SelectedFileIdentity {
            volume: info.VolumeSerialNumber,
            object: info.FileId.Identifier,
        })
    }

    fn attributes(file: &File) -> Result<FILE_ATTRIBUTE_TAG_INFO> {
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
        ensure!(
            file.metadata()?.is_file() && info.FileAttributes & UNAVAILABLE == 0,
            "selected object is unavailable or not a regular file"
        );
        Ok(info)
    }

    pub(super) fn open(path: &Path) -> Result<(File, SelectedFileIdentity)> {
        ancestors(path)?;
        let file = open_path(path, FILE_FLAG_OPEN_REPARSE_POINT.0)?;
        let tag = attributes(&file)?;
        let redirected = tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0;
        ensure!(
            !redirected || tag.ReparseTag & !CLOUD_VARIANTS == CLOUD_TAG,
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
        ancestors(path)?;
        Ok((file, id))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;

    pub(super) fn open(_: &Path) -> Result<(File, SelectedFileIdentity)> {
        anyhow::bail!("native selected file identity unavailable");
    }
}
