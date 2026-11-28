//! Unicode QUERY2 / LIST2 fields from voidtools' Everything IPC SDK.
use anyhow::{ensure, Context, Result};

const MATCH_PATH: u32 = 0x04;
const REGEX: u32 = 0x08;
const FULL_PATH: u32 = 0x04;
const SORT_NAME_ASCENDING: u32 = 1;
const FOLDER_OR_ROOT: u32 = 0x03;
const LIST_HEADER: usize = 20;
const ITEM_HEADER: usize = 8;
const MAX_PATH_UNITS: usize = 32_767;

pub(crate) struct Reply {
    pub(crate) paths: Vec<(Vec<u16>, bool)>,
    pub(crate) truncated: bool,
}

pub(crate) fn pattern(query: &str, roots: &[&str]) -> Result<String> {
    ensure!(
        !roots.is_empty() && roots.len() <= 8,
        "Everything requires search roots"
    );
    ensure!(
        !query.trim().is_empty() && query.len() <= 256 && !query.chars().any(char::is_control),
        "invalid Everything search text"
    );
    let mut pattern = String::from(r"(?i)^(?:");
    for (index, root) in roots.iter().enumerate() {
        ensure!(
            !root.is_empty() && !root.chars().any(char::is_control),
            "invalid Everything search root"
        );
        if index > 0 {
            pattern.push('|');
        }
        escape(root.trim_end_matches('\\'), &mut pattern);
        pattern.push_str(r"\\");
    }
    pattern.push_str(r")(?:[^\\]+\\)*");
    for word in query.split_whitespace() {
        pattern.push_str(r"(?=[^\\]*");
        escape(word, &mut pattern);
        pattern.push(')');
    }
    pattern.push_str(r"[^\\]+$");
    ensure!(
        pattern.encode_utf16().count() <= MAX_PATH_UNITS,
        "Everything search is too long"
    );
    Ok(pattern)
}

fn escape(text: &str, pattern: &mut String) {
    for character in text.chars() {
        if r"\.^$|?*+()[]{}".contains(character) {
            pattern.push('\\');
        }
        pattern.push(character);
    }
}

pub(crate) fn request(window: u32, id: u32, pattern: &str, limit: u32) -> Vec<u8> {
    [
        window,
        id,
        MATCH_PATH | REGEX,
        0,
        limit,
        FULL_PATH,
        SORT_NAME_ASCENDING,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .chain(pattern.encode_utf16().chain([0]).flat_map(u16::to_le_bytes))
    .collect()
}

fn dword(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub(crate) fn parse_reply(bytes: &[u8], limit: u32) -> Result<Reply> {
    parse_list(bytes, limit).context("invalid Everything IPC reply")
}

fn parse_list(bytes: &[u8], limit: u32) -> Option<Reply> {
    let total = dword(bytes, 0)?;
    let count = dword(bytes, 4)?;
    if count > total || count > limit || dword(bytes, 8)? != 0 {
        return None;
    }
    if count != 0 && dword(bytes, 12)? != FULL_PATH {
        return None;
    }
    let table_end = LIST_HEADER.checked_add((count as usize).checked_mul(ITEM_HEADER)?)?;
    let table = bytes
        .get(LIST_HEADER..table_end)?
        .as_chunks::<ITEM_HEADER>()
        .0;
    let mut paths = Vec::with_capacity(count as usize);
    let mut consumed = table_end;
    for item in table {
        let flags = dword(item, 0)?;
        let offset = dword(item, 4)? as usize;
        if offset < consumed {
            return None;
        }
        let length = dword(bytes, offset)? as usize;
        if length == 0 || length > MAX_PATH_UNITS {
            return None;
        }
        let start = offset.checked_add(4)?;
        let end = start.checked_add(length.checked_mul(2)?)?;
        consumed = end.checked_add(2)?;
        if bytes.get(end..consumed)? != [0, 0] {
            return None;
        }
        let path: Vec<_> = bytes
            .get(start..end)?
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect();
        if path.contains(&0) {
            return None;
        }
        paths.push((path, flags & FOLDER_OR_ROOT != 0));
    }
    Some(Reply {
        paths,
        truncated: total > count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_scopes_each_root_and_escapes_literal_words() {
        let regex = pattern("日志 +.[x] |foo", &[r"C:\Working (2)\", r"D:\用户"]).unwrap();
        assert_eq!(
            regex,
            r"(?i)^(?:C:\\Working \(2\)\\|D:\\用户\\)(?:[^\\]+\\)*(?=[^\\]*日志)(?=[^\\]*\+\.\[x\])(?=[^\\]*\|foo)[^\\]+$"
        );
        assert!(pattern("x", &[]).is_err());
        assert!(pattern("x\0", &[r"C:\"]).is_err());
        let request = request(22, 33, &regex, 64);
        assert_eq!(
            (dword(&request, 0), dword(&request, 4), dword(&request, 8)),
            (Some(22), Some(33), Some(12))
        );
        assert_eq!(dword(&request, 16), Some(64));
        assert_eq!(dword(&request, 20), Some(4));
        assert_eq!(&request[request.len() - 2..], &[0, 0]);
    }

    #[test]
    fn reply_preserves_unicode_type_and_reports_truncation() {
        let path: Vec<_> = r"C:\测试\截图📋.txt".encode_utf16().collect();
        let reply: Vec<_> = [3_u32, 1, 0, FULL_PATH, 1, 1, 28, path.len() as u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .chain(path.iter().copied().chain([0]).flat_map(u16::to_le_bytes))
            .collect();
        let parsed = parse_reply(&reply, 1).unwrap();
        assert_eq!(parsed.paths, [(path, true)]);
        assert!(parsed.truncated);
        assert!(parse_reply(&reply, 0).is_err());
        assert!(parse_reply(&reply[..reply.len() - 1], 1).is_err());
        let mut invalid = reply;
        invalid[24..28].copy_from_slice(&20_u32.to_le_bytes());
        assert!(parse_reply(&invalid, 1).is_err());
    }
}
