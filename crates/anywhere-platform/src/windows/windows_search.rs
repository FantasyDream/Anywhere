//! Native Windows Search (`SystemIndex`) backend via COM OLE DB (`Search.CollatorDSO`).
use super::shell::Apartment;
use crate::{
    FileIndexResults, FileSearchBackend, FileSearchEngine, FileSearchHit, FileSearchQuery,
};
use anyhow::{bail, ensure, Result};
use std::{
    borrow::Cow,
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Component, Path, PathBuf, Prefix},
    time::{Duration, Instant},
};
use windows::{
    core::{w, Interface, GUID, HSTRING, PCWSTR},
    Win32::{
        Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY,
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Search::{
                CSearchManager, IAccessor, ICommandText, IDBCreateCommand, IDBCreateSession,
                IDBInitialize, IDataInitialize, IRowset, ISearchManager, DBACCESSOR_ROWDATA,
                DBBINDING, DBMEMOWNER_CLIENTOWNED, DBPART_STATUS, DBPART_VALUE, DBSTATUS_S_OK,
                DBTYPE_UI4, DBTYPE_WSTR, HACCESSOR,
            },
        },
    },
};

const CLSID_MSDAINITIALIZE: GUID = GUID::from_u128(0x2206cdb0_19c1_11d1_89e0_00c04fd7a829);
const DBGUID_DEFAULT: GUID = GUID::from_u128(0xc8b521fb_5cf3_11ce_ade5_00aa0044773d);
const CONNECTION_STRING: PCWSTR =
    w!("Provider=Search.CollatorDSO;Extended Properties='Application=Windows';");
const PATH_BUFFER_CHARS: usize = 1024;
const MAX_WINDOWS_SEARCH_RESULTS: usize = 512;

#[repr(C)]
struct RowBuffer {
    path_status: u32,
    attrs_status: u32,
    attrs: u32,
    path: [u16; PATH_BUFFER_CHARS],
}

pub(super) struct WindowsSearchBackend;

impl FileSearchBackend for WindowsSearchBackend {
    fn engine(&self) -> FileSearchEngine {
        FileSearchEngine::SystemIndex
    }

    fn is_available(&self) -> bool {
        let Ok(_apartment) = Apartment::open() else {
            return false;
        };
        let Ok(manager): windows::core::Result<ISearchManager> =
            (unsafe { CoCreateInstance(&CSearchManager, None, CLSCTX_INPROC_SERVER) })
        else {
            return false;
        };
        unsafe { manager.GetCatalog(w!("SystemIndex")) }.is_ok()
    }

    fn search(
        &self,
        query: &FileSearchQuery<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        search(
            query.text,
            query.roots,
            query.limit,
            query.timeout,
            cancelled,
        )
    }
}

pub(super) fn search(
    query: &str,
    roots: &[PathBuf],
    limit: usize,
    timeout: Duration,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<FileIndexResults> {
    if cancelled() || limit == 0 || roots.is_empty() {
        return Ok(FileIndexResults::default());
    }
    let limit = limit.clamp(1, MAX_WINDOWS_SEARCH_RESULTS);
    let scoped_roots: Vec<_> = roots
        .iter()
        .map(|root| (root.as_path(), normalized_scope_path(root)))
        .collect();
    let Some(sql) = build_query(query, &scoped_roots, limit) else {
        return Ok(FileIndexResults::default());
    };
    let deadline = Instant::now() + timeout;
    let _apartment = Apartment::open()?;

    let data_init: IDataInitialize =
        unsafe { CoCreateInstance(&CLSID_MSDAINITIALIZE, None, CLSCTX_INPROC_SERVER) }?;
    let mut data_source = None;
    unsafe {
        data_init.GetDataSource(
            None,
            CLSCTX_INPROC_SERVER.0,
            CONNECTION_STRING,
            &IDBInitialize::IID,
            &mut data_source,
        )?;
    }
    let db_init: IDBInitialize = data_source
        .ok_or_else(|| anyhow::anyhow!("Windows Search data source unavailable"))?
        .cast()?;
    unsafe { db_init.Initialize()? };

    ensure!(!cancelled(), "search cancelled");
    let create_session: IDBCreateSession = db_init.cast()?;
    let session: IDBCreateCommand =
        unsafe { create_session.CreateSession(None, &IDBCreateCommand::IID)? }.cast()?;
    let command: ICommandText =
        unsafe { session.CreateCommand(None, &ICommandText::IID)? }.cast()?;
    let sql_wide = HSTRING::from(sql);
    unsafe { command.SetCommandText(&DBGUID_DEFAULT, &sql_wide)? };

    ensure!(!cancelled(), "search cancelled");
    let mut rowset_unknown = None;
    unsafe {
        command.Execute(None, &IRowset::IID, None, None, Some(&mut rowset_unknown))?;
    }
    let Some(rowset_unknown) = rowset_unknown else {
        return Ok(FileIndexResults::default());
    };
    let rowset: IRowset = rowset_unknown.cast()?;
    let accessor: IAccessor = rowset.cast()?;

    let bindings = [
        DBBINDING {
            iOrdinal: 1,
            obValue: std::mem::offset_of!(RowBuffer, path),
            obStatus: std::mem::offset_of!(RowBuffer, path_status),
            dwPart: (DBPART_VALUE.0 | DBPART_STATUS.0) as u32,
            dwMemOwner: DBMEMOWNER_CLIENTOWNED.0 as u32,
            cbMaxLen: PATH_BUFFER_CHARS * size_of::<u16>(),
            wType: DBTYPE_WSTR.0 as u16,
            ..Default::default()
        },
        DBBINDING {
            iOrdinal: 2,
            obValue: std::mem::offset_of!(RowBuffer, attrs),
            obStatus: std::mem::offset_of!(RowBuffer, attrs_status),
            dwPart: (DBPART_VALUE.0 | DBPART_STATUS.0) as u32,
            dwMemOwner: DBMEMOWNER_CLIENTOWNED.0 as u32,
            cbMaxLen: size_of::<u32>(),
            wType: DBTYPE_UI4.0 as u16,
            ..Default::default()
        },
    ];
    let mut haccessor = HACCESSOR::default();
    unsafe {
        accessor.CreateAccessor(
            DBACCESSOR_ROWDATA.0 as u32,
            bindings.len(),
            bindings.as_ptr(),
            size_of::<RowBuffer>(),
            &mut haccessor,
            None,
        )?;
    }
    struct AccessorGuard<'a>(&'a IAccessor, HACCESSOR);
    impl Drop for AccessorGuard<'_> {
        fn drop(&mut self) {
            unsafe {
                let _ = self.0.ReleaseAccessor(self.1, None);
            }
        }
    }
    let _guard = AccessorGuard(&accessor, haccessor);

    let mut hits = Vec::new();
    let mut truncated = false;
    loop {
        if cancelled() {
            bail!("search cancelled");
        }
        if Instant::now() >= deadline {
            truncated = true;
            break;
        }
        let mut obtained = 0usize;
        let mut single_row = 0usize;
        let mut single_ptr: *mut usize = &mut single_row;
        let status = unsafe {
            rowset.GetNextRows(0, 0, &mut obtained, std::slice::from_mut(&mut single_ptr))
        };
        if status.is_err() || obtained == 0 {
            break;
        }
        let mut buffer: RowBuffer = unsafe { std::mem::zeroed() };
        let data_ok = unsafe {
            rowset
                .GetData(
                    single_row,
                    haccessor,
                    std::ptr::from_mut(&mut buffer).cast(),
                )
                .is_ok()
        };
        unsafe {
            let _ = rowset.ReleaseRows(
                1,
                &single_row,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
        if !data_ok || buffer.path_status != DBSTATUS_S_OK.0 as u32 {
            continue;
        }
        let end = buffer
            .path
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.path.len());
        let raw_path = PathBuf::from(OsString::from_wide(&buffer.path[..end]));
        let is_directory = if buffer.attrs_status == DBSTATUS_S_OK.0 as u32 {
            (buffer.attrs & FILE_ATTRIBUTE_DIRECTORY.0) != 0
        } else {
            raw_path.is_dir()
        };
        if let Some(path) = scoped_path(&raw_path, &scoped_roots) {
            hits.push(FileSearchHit { path, is_directory });
            if hits.len() >= limit {
                truncated = true;
                break;
            }
        }
    }
    Ok(FileIndexResults { hits, truncated })
}

fn build_query(query: &str, roots: &[(&Path, Cow<'_, Path>)], limit: usize) -> Option<String> {
    if query.len() > 256 || query.chars().any(char::is_control) {
        return None;
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() || words.len() > 16 {
        return None;
    }
    let mut scope_clauses = Vec::new();
    for (_, root) in roots {
        let scope = scope_uri(root)?;
        scope_clauses.push(format!("SCOPE='{}'", escape_sql_literal(&scope)));
    }
    if scope_clauses.is_empty() {
        return None;
    }
    let mut conditions = Vec::with_capacity(words.len() + 1);
    conditions.push(format!("({})", scope_clauses.join(" OR ")));
    for word in words {
        conditions.push(format!(
            "System.FileName LIKE '%{}%'",
            escape_like_pattern(word)
        ));
    }
    Some(format!(
        "SELECT TOP {} System.ItemPathDisplay, System.FileAttributes FROM SystemIndex WHERE {} ORDER BY System.DateModified DESC",
        limit,
        conditions.join(" AND ")
    ))
}

fn scope_uri(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }
    let normalized = text.replace('\\', "/");
    Some(format!("file:{normalized}"))
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_like_pattern(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\'' => out.push_str("''"),
            '%' => out.push_str("[%]"),
            '_' => out.push_str("[_]"),
            '[' => out.push_str("[[]"),
            other => out.push(other),
        }
    }
    out
}

fn normalized_scope_path(path: &Path) -> Cow<'_, Path> {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Cow::Borrowed(path);
    };
    match prefix.kind() {
        Prefix::VerbatimDisk(_) => path
            .to_str()
            .and_then(|value| value.strip_prefix(r"\\?\"))
            .map_or(Cow::Borrowed(path), |value| Cow::Borrowed(Path::new(value))),
        Prefix::VerbatimUNC(_, _) => path
            .to_str()
            .and_then(|value| value.strip_prefix(r"\\?\UNC\"))
            .map_or(Cow::Borrowed(path), |value| {
                Cow::Owned(PathBuf::from(format!(r"\\{value}")))
            }),
        _ => Cow::Borrowed(path),
    }
}

fn scoped_path(path: &Path, roots: &[(&Path, Cow<'_, Path>)]) -> Option<PathBuf> {
    let path = normalized_scope_path(path);
    roots.iter().find_map(|(root, indexed)| {
        let mut remaining = path.components();
        for expected in indexed.components() {
            let actual = remaining.next()?;
            if !actual
                .as_os_str()
                .eq_ignore_ascii_case(expected.as_os_str())
            {
                return None;
            }
        }
        remaining
            .clone()
            .all(|part| matches!(part, Component::Normal(_)))
            .then(|| root.join(remaining.as_path()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_search_sql_escapes_wildcards_quotes_and_scopes() {
        let root = Path::new(r"\\?\C:\Users\O'Brien\Docs");
        let roots = [(root, normalized_scope_path(root))];
        let sql = build_query("report_100% [draft]", &roots, 64).unwrap();
        assert!(sql.contains(
            "SELECT TOP 64 System.ItemPathDisplay, System.FileAttributes FROM SystemIndex"
        ));
        assert!(sql.contains("SCOPE='file:C:/Users/O''Brien/Docs'"));
        assert!(sql.contains("System.FileName LIKE '%report[_]100[%]%'"));
        assert!(sql.contains("System.FileName LIKE '%[[]draft]%'"));
    }

    #[test]
    fn windows_search_scoped_path_enforces_component_boundaries() {
        let root = Path::new(r"\\?\C:\Data");
        let roots = [(root, normalized_scope_path(root))];
        assert_eq!(
            scoped_path(Path::new(r"c:\data\sub\报告.txt"), &roots),
            Some(root.join(r"sub\报告.txt"))
        );
        assert_eq!(
            scoped_path(Path::new(r"C:\Database\报告.txt"), &roots),
            None
        );
        assert_eq!(
            scoped_path(Path::new(r"C:\Data\..\outside.txt"), &roots),
            None
        );
    }

    #[test]
    fn windows_search_executes_against_system_index_when_service_is_available() {
        if !WindowsSearchBackend.is_available() {
            return;
        }
        let Some(home) = crate::paths::home_directory() else {
            return;
        };
        let root = home.canonicalize().unwrap();
        let result = search("desktop", &[root], 16, Duration::from_secs(3), &|| false);
        assert!(result.is_ok(), "Windows Search query failed: {result:?}");
    }
}
