mod icon;

pub(super) use icon::icon;

use super::shell::{self, Apartment};
use crate::{BundleApplication, InstalledApplications};
use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    os::windows::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
};
use windows::{
    core::{Interface, PCWSTR, PWSTR},
    Win32::{
        Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
        System::Com::{
            CoCreateInstance, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ,
        },
        UI::Shell::{
            ApplicationActivationManager, BHID_EnumItems, FOLDERID_AppsFolder,
            FOLDERID_CommonPrograms, FOLDERID_Programs, IApplicationActivationManager,
            IEnumShellItems, IShellItem, IShellLinkDataList, IShellLinkW, SHGetKnownFolderItem,
            SHGetKnownFolderPath, ShellLink, AO_NONE, KF_FLAG_DEFAULT, SIGDN_NORMALDISPLAY,
            SIGDN_PARENTRELATIVEPARSING, SLDF_RUNAS_USER,
        },
    },
};

const APPS_FOLDER: &str = r"shell:AppsFolder\";
const MAX_ITEMS: usize = 4000;
const MAX_VISITED: usize = 20_000;
const MAX_DEPTH: usize = 8;

pub(super) fn start_menu_roots() -> Vec<PathBuf> {
    [FOLDERID_Programs, FOLDERID_CommonPrograms]
        .into_iter()
        .filter_map(|folder| unsafe { SHGetKnownFolderPath(&folder, KF_FLAG_DEFAULT, None) }.ok())
        .map(|path| PathBuf::from(take_shell_string(path)))
        .filter(|path| path.is_absolute())
        .collect()
}

fn take_shell_string(value: PWSTR) -> OsString {
    let string = OsString::from_wide(unsafe { value.as_wide() });
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    string
}

pub(super) fn packaged_target(path: &str) -> Option<&str> {
    let id = path.strip_prefix(APPS_FOLDER)?;
    let (family, app) = id.split_once('!')?;
    let (package, publisher) = family.rsplit_once('_')?;
    let valid = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    };
    (id.len() <= 256
        && valid(package)
        && publisher.len() == 13
        && publisher.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && valid(app))
    .then_some(id)
}

pub(super) fn launch_packaged(id: &str) -> anyhow::Result<()> {
    let _apartment = Apartment::open()?;
    let id = shell::wide(id.as_ref())?;
    let manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER) }?;
    unsafe { manager.ActivateApplication(PCWSTR(id.as_ptr()), PCWSTR::null(), AO_NONE) }?;
    Ok(())
}

fn packaged_entries() -> windows::core::Result<Vec<BundleApplication>> {
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) }?;
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems) }?;
    let mut applications = Vec::new();
    for _ in 0..MAX_ITEMS {
        let mut slot = [None];
        unsafe { items.Next(&mut slot, None) }?;
        let Some(item) = slot[0].take() else { break };
        let Ok(id) = (unsafe { item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING) }) else {
            continue;
        };
        let id = take_shell_string(id);
        let Some(id) = id.to_str() else { continue };
        let path = format!("{APPS_FOLDER}{id}");
        if packaged_target(&path).is_none() {
            continue;
        }
        let Ok(name) = (unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }) else {
            continue;
        };
        let name = take_shell_string(name).to_string_lossy().into_owned();
        let aliases = id
            .split_once('!')
            .and_then(|(family, _)| family.rsplit_once('_'))
            .map(|(package, _)| package.trim_start_matches("Microsoft.").to_owned())
            .into_iter()
            .collect();
        applications.push(entry(
            format!("windows.aumid:{}", id.to_ascii_lowercase()),
            name,
            path,
            aliases,
        ));
    }
    Ok(applications)
}

pub(super) fn is_launch_file(path: &Path) -> bool {
    ["lnk", "url", "exe"]
        .iter()
        .any(|suffix| shell::has_extension(path, suffix))
}

pub(super) fn file_entry(path: &Path) -> Option<BundleApplication> {
    let name = if path.is_dir() {
        path.file_name()?
    } else {
        path.file_stem()?
    };
    let kind = if shell::has_extension(path, "lnk") {
        "shortcut"
    } else {
        "path"
    };
    let value = path.to_str()?.to_owned();
    Some(entry(
        format!("windows.{kind}:{}", path_key(path)),
        name.to_str()?.to_owned(),
        value,
        Vec::new(),
    ))
}

fn entry(bundle_id: String, name: String, path: String, aliases: Vec<String>) -> BundleApplication {
    BundleApplication {
        bundle_id,
        name,
        localized_names: Default::default(),
        aliases,
        path: Some(path),
        icon: None,
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LaunchIdentity {
    target: String,
    arguments: String,
    directory: String,
    run_as: bool,
}

impl LaunchIdentity {
    fn new(path: &Path, arguments: String, directory: &Path, run_as: bool) -> Self {
        let directory = if !directory.is_absolute() {
            path.parent().unwrap_or_else(|| Path::new(""))
        } else {
            directory
        };
        Self {
            target: path_key(path),
            arguments,
            directory: path_key(directory),
            run_as,
        }
    }
}

fn path_key(path: &Path) -> String {
    dunce::simplified(path)
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

fn shortcut_identity(path: &Path) -> anyhow::Result<Option<(LaunchIdentity, Option<String>)>> {
    let path = shell::wide(path.as_os_str())?;
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }?;
    let persist: IPersistFile = link.cast()?;
    unsafe { persist.Load(PCWSTR(path.as_ptr()), STGM_READ) }?;
    let mut buffer = vec![0; 32_768];
    unsafe { link.GetPath(&mut buffer, std::ptr::null_mut(), 0) }?;
    let target = PathBuf::from(wide_buffer(&buffer));
    if !target.is_absolute() {
        return Ok(None);
    }
    let target_name = target
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned);
    buffer.fill(0);
    unsafe { link.GetArguments(&mut buffer) }?;
    let arguments = wide_buffer(&buffer).to_string_lossy().into_owned();
    buffer.fill(0);
    unsafe { link.GetWorkingDirectory(&mut buffer) }?;
    let working_directory = PathBuf::from(wide_buffer(&buffer));
    let run_as = link
        .cast::<IShellLinkDataList>()
        .and_then(|data| unsafe { data.GetFlags() })
        .is_ok_and(|flags| flags & SLDF_RUNAS_USER.0 as u32 != 0);
    Ok(Some((
        LaunchIdentity::new(&target, arguments, &working_directory, run_as),
        target_name,
    )))
}

fn wide_buffer(buffer: &[u16]) -> OsString {
    OsString::from_wide(buffer.split(|unit| *unit == 0).next().unwrap_or_default())
}

pub(super) fn resolve_launch_executable(path: &Path) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(path.is_absolute(), "application path must be absolute");
    if shell::has_extension(path, "exe") && path.is_file() {
        return Ok(dunce::simplified(path).to_path_buf());
    }
    if shell::has_extension(path, "lnk") && path.is_file() {
        let _apartment = Apartment::open()?;
        let (identity, _) = shortcut_identity(path)?
            .ok_or_else(|| anyhow::anyhow!("shortcut target is unavailable"))?;
        let target = PathBuf::from(&identity.target);
        anyhow::ensure!(
            target.is_absolute() && target.is_file() && shell::has_extension(&target, "exe"),
            "shortcut does not point to an executable"
        );
        return Ok(target);
    }
    anyhow::bail!("application target is not a launchable executable")
}

fn canonical_bundle_id(executable: &Path) -> Option<&'static str> {
    let stem = executable.file_stem()?.to_str()?.to_ascii_lowercase();
    match stem.as_str() {
        "code" => Some("com.microsoft.VSCode"),
        "code - insiders" => Some("com.microsoft.VSCodeInsiders"),
        "vscodium" => Some("com.vscodium"),
        "cursor" => Some("com.todesktop.230313mzl4w4u92"),
        "windsurf" => Some("com.exafunction.windsurf"),
        "obsidian" => Some("md.obsidian"),
        "sublime_text" => Some("com.sublimetext.4"),
        "idea64" | "idea" => Some("com.jetbrains.intellij"),
        "pycharm64" | "pycharm" => Some("com.jetbrains.pycharm"),
        "webstorm64" | "webstorm" => Some("com.jetbrains.WebStorm"),
        "goland64" | "goland" => Some("com.jetbrains.goland"),
        "rustrover64" | "rustrover" => Some("com.jetbrains.rustrover"),
        "clion64" | "clion" => Some("com.jetbrains.CLion"),
        "rider64" | "rider" => Some("com.jetbrains.rider"),
        "phpstorm64" | "phpstorm" => Some("com.jetbrains.PhpStorm"),
        "rubymine64" | "rubymine" => Some("com.jetbrains.rubymine"),
        "datagrip64" | "datagrip" => Some("com.jetbrains.datagrip"),
        "studio64" => Some("com.google.android.studio"),
        _ => None,
    }
}

pub(super) fn discover(start_menu: &[PathBuf], custom: &[PathBuf]) -> InstalledApplications {
    let apartment = Apartment::open().ok();
    let custom_directories = custom
        .iter()
        .filter(|path| path.is_absolute() && path.is_dir())
        .cloned()
        .collect::<Vec<_>>();
    let custom_files = custom
        .iter()
        .filter(|path| path.is_absolute() && path.is_file() && is_launch_file(path))
        .cloned();
    let start_shortcuts = walk(start_menu, false);
    let custom_children = walk(&custom_directories, true);
    let mut paths = HashSet::new();
    let mut targets = BTreeMap::<LaunchIdentity, usize>::new();
    let mut claimed_bundle_ids = HashSet::new();
    let mut applications: Vec<BundleApplication> = Vec::new();
    let entries = start_shortcuts
        .iter()
        .chain(&custom_directories)
        .cloned()
        .chain(custom_files)
        .chain(custom_children);
    for path in entries {
        if !paths.insert(path_key(&path)) {
            continue;
        }
        let Some(mut application) = file_entry(&path) else {
            continue;
        };
        let identity = if shell::has_extension(&path, "lnk") && apartment.is_some() {
            shortcut_identity(&path).ok().flatten()
        } else if path.is_dir() || shell::has_extension(&path, "exe") {
            Some((
                LaunchIdentity::new(&path, String::new(), Path::new(""), false),
                None,
            ))
        } else {
            None
        };
        if let Some((identity, target_name)) = identity {
            if let Some(canonical) = canonical_bundle_id(Path::new(&identity.target)) {
                if claimed_bundle_ids.insert(canonical) {
                    application.bundle_id = canonical.to_owned();
                }
            }
            application.aliases.extend(target_name);
            if let Some(index) = targets.get(&identity) {
                let winner = &mut applications[*index];
                winner.aliases.push(application.name);
                winner.aliases.extend(application.aliases);
                continue;
            }
            targets.insert(identity, applications.len());
        }
        applications.push(application);
    }
    if apartment.is_some() {
        match packaged_entries() {
            Ok(packaged) => applications.extend(packaged),
            Err(error) => tracing::debug!(%error, "windows_apps_folder_unavailable"),
        }
    }
    InstalledApplications {
        applications,
        skipped: 0,
    }
}

fn walk(roots: &[PathBuf], include_exes: bool) -> Vec<PathBuf> {
    let mut applications = Vec::new();
    let mut visited = HashSet::new();
    let mut remaining = MAX_VISITED;
    let mut pending = roots
        .iter()
        .rev()
        .map(|root| (root.clone(), 0))
        .collect::<Vec<_>>();
    while let Some((directory, depth)) = pending.pop() {
        if remaining == 0 || applications.len() >= MAX_ITEMS {
            break;
        }
        if !visited.insert(path_key(&directory)) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries.take(remaining).flatten().collect::<Vec<_>>();
        remaining -= entries.len();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        let mut children = Vec::new();
        for entry in entries {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
                continue;
            }
            let path = entry.path();
            if metadata.is_dir() && depth < MAX_DEPTH {
                children.push((path, depth + 1));
            } else if metadata.is_file()
                && (shell::has_extension(&path, "lnk")
                    || shell::has_extension(&path, "url")
                    || include_exes && shell::has_extension(&path, "exe"))
            {
                applications.push(path);
                if applications.len() >= MAX_ITEMS {
                    break;
                }
            }
        }
        pending.extend(children.into_iter().rev());
    }
    applications
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_shortcut(path: &Path, target: &Path, arguments: &str) {
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
        let target = shell::wide(target.as_os_str()).unwrap();
        let arguments = shell::wide(arguments.as_ref()).unwrap();
        let path = shell::wide(path.as_os_str()).unwrap();
        unsafe {
            link.SetPath(PCWSTR(target.as_ptr())).unwrap();
            link.SetArguments(PCWSTR(arguments.as_ptr())).unwrap();
            link.cast::<IPersistFile>()
                .unwrap()
                .Save(PCWSTR(path.as_ptr()), true)
                .unwrap();
        }
    }

    #[test]
    fn windows_catalog_indexes_custom_paths_and_merges_equivalent_shortcuts() {
        let _apartment = Apartment::open().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("Anywhere-Test-Portable");
        let nested = root.join("nested/tools");
        std::fs::create_dir_all(&nested).unwrap();
        let executable = nested.join("Anywhere-Test-Tool.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let shortcut = fixture.path().join("Anywhere-Test-Shortcut.lnk");
        let duplicate = nested.join("Anywhere-Test-Alias.lnk");
        let distinct = nested.join("Anywhere-Test-Arguments.lnk");
        save_shortcut(&shortcut, &executable, "");
        save_shortcut(&duplicate, &executable, "");
        save_shortcut(&distinct, &executable, "--profile Work");

        let report = discover(&[], &[root, shortcut]);
        let fixture_apps = report
            .applications
            .iter()
            .filter(|app| app.name.starts_with("Anywhere-Test-"))
            .collect::<Vec<_>>();
        assert_eq!(fixture_apps.len(), 3);
        let primary = fixture_apps
            .iter()
            .find(|app| app.name == "Anywhere-Test-Shortcut")
            .unwrap();
        assert!(primary
            .aliases
            .iter()
            .any(|name| name == "Anywhere-Test-Alias"));
        assert!(primary
            .aliases
            .iter()
            .any(|name| name == "Anywhere-Test-Tool"));
        assert!(fixture_apps
            .iter()
            .any(|app| app.name == "Anywhere-Test-Arguments"));
        assert!(fixture_apps
            .iter()
            .any(|app| app.name == "Anywhere-Test-Portable"));
    }

    #[test]
    fn windows_catalog_reads_shell_icons_as_bgra() {
        let executable = std::env::current_exe().unwrap();
        let image = icon(executable.to_str().unwrap(), 48).unwrap().unwrap();
        assert_eq!(image.bgra.len(), (image.width * image.height * 4) as usize);
        assert!(image
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0));
    }
}
