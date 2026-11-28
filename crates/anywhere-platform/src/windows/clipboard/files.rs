use super::LockedClipboard;
use crate::{
    clipboard::{ClipboardFormat, ClipboardPayload, FILES, MAX_FILE_LIST_ITEMS, MAX_PAYLOAD_BYTES},
    ContextReadError,
};
use anyhow::{ensure, Result};
use windows_sys::Win32::{
    Globalization::{MultiByteToWideChar, CP_ACP, MB_ERR_INVALID_CHARS},
    System::{
        DataExchange::GetClipboardData,
        Memory::{GlobalLock, GlobalSize},
    },
    UI::Shell::DROPFILES,
};

pub(super) const FORMAT: u32 = 15; // CF_HDROP

pub(super) fn read_open() -> Result<ClipboardPayload> {
    let handle = unsafe { GetClipboardData(FORMAT) };
    ensure!(!handle.is_null(), ContextReadError::Unavailable);
    let size = unsafe { GlobalSize(handle) };
    ensure!(
        size >= size_of::<DROPFILES>(),
        ContextReadError::Unavailable
    );
    ensure!(size <= MAX_PAYLOAD_BYTES, ContextReadError::TooLarge);
    let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
    ensure!(!pointer.is_null(), ContextReadError::Unavailable);
    let _locked = LockedClipboard(handle);
    // GlobalLock makes these bytes readable until the guard unlocks the handle.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, size) };
    file_payload(bytes)
}

fn file_payload(bytes: &[u8]) -> Result<ClipboardPayload> {
    ensure!(
        bytes.len() >= size_of::<DROPFILES>(),
        ContextReadError::Unavailable
    );
    // DROPFILES contains only integer fields, and the byte buffer may be unaligned.
    let header = unsafe { bytes.as_ptr().cast::<DROPFILES>().read_unaligned() };
    let offset = header.pFiles as usize;
    ensure!(
        (size_of::<DROPFILES>()..bytes.len()).contains(&offset),
        ContextReadError::Unavailable
    );
    let bytes = &bytes[offset..];
    let data = if header.fWide != 0 {
        ensure!(offset.is_multiple_of(2), ContextReadError::Unavailable);
        let (units, _) = bytes.as_chunks::<2>();
        encode_file_list(units, [0, 0], |path| {
            let units: Vec<_> = path.iter().copied().map(u16::from_le_bytes).collect();
            String::from_utf16(&units).map_err(|_| ContextReadError::Unavailable.into())
        })?
    } else {
        encode_file_list(bytes, 0, decode_ansi_path)?
    };
    let payload = ClipboardPayload {
        formats: vec![ClipboardFormat {
            name: FILES.into(),
            data,
        }],
    };
    payload.validate()?;
    Ok(payload)
}

fn encode_file_list<T: PartialEq>(
    units: &[T],
    nul: T,
    decode_path: impl Fn(&[T]) -> Result<String>,
) -> Result<Vec<u8>> {
    let mut data = vec![b'['];
    for (index, entry) in units.split_inclusive(|unit| *unit == nul).enumerate() {
        let path = entry
            .strip_suffix(std::slice::from_ref(&nul))
            .ok_or(ContextReadError::Unavailable)?;
        if path.is_empty() {
            data.push(b']');
            return Ok(data);
        }
        ensure!(index < MAX_FILE_LIST_ITEMS, ContextReadError::TooManyItems);
        ensure!(path.len() <= 32_767, ContextReadError::TooLarge);
        if index > 0 {
            data.push(b',');
        }
        serde_json::to_writer(&mut data, &decode_path(path)?)?;
        ensure!(data.len() < MAX_PAYLOAD_BYTES, ContextReadError::TooLarge);
    }
    Err(ContextReadError::Unavailable.into())
}

fn decode_ansi_path(path: &[u8]) -> Result<String> {
    // The system ANSI code page produces at most one UTF-16 unit per input byte.
    let mut wide = vec![0; path.len()];
    let length = unsafe {
        MultiByteToWideChar(
            CP_ACP,
            MB_ERR_INVALID_CHARS,
            path.as_ptr(),
            path.len() as i32,
            wide.as_mut_ptr(),
            wide.len() as i32,
        )
    };
    ensure!(length > 0, ContextReadError::Unavailable);
    String::from_utf16(&wide[..length as usize]).map_err(|_| ContextReadError::Unavailable.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dropfiles_header(wide: bool) -> Vec<u8> {
        let mut bytes = vec![0; size_of::<DROPFILES>()];
        bytes[..4].copy_from_slice(&(size_of::<DROPFILES>() as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&u32::from(wide).to_le_bytes());
        bytes
    }

    fn wide_dropfiles(list: &str) -> Vec<u8> {
        let mut bytes = dropfiles_header(true);
        bytes.extend(list.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    #[test]
    fn windows_clipboard_file_list_supports_unicode_and_ansi() {
        let wide = wide_dropfiles("C:\\测试目录\\file one.txt\0D:\\资料😀\0\0");
        assert_eq!(
            file_payload(&wide).unwrap().files().unwrap(),
            [r"C:\测试目录\file one.txt", r"D:\资料😀"],
        );

        let mut ansi = dropfiles_header(false);
        ansi.extend_from_slice(b"C:\\file one.txt\0D:\\folder\0\0");
        assert_eq!(
            file_payload(&ansi).unwrap().files().unwrap(),
            [r"C:\file one.txt", r"D:\folder"],
        );
    }

    #[test]
    fn windows_clipboard_file_list_rejects_malformed_data() {
        let mut invalid_offset = wide_dropfiles("C:\\file.txt\0\0");
        invalid_offset[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut invalid_utf16 = wide_dropfiles("C:\\");
        invalid_utf16.extend([0x00, 0xD8, 0, 0, 0, 0]);
        for bytes in [
            vec![0; 4],
            invalid_offset,
            invalid_utf16,
            wide_dropfiles("C:\\file.txt\0"),
            wide_dropfiles("C:file.txt\0\0"),
            wide_dropfiles("\0\0"),
        ] {
            assert!(file_payload(&bytes).is_err());
        }
    }

    #[test]
    fn windows_clipboard_file_list_enforces_item_limit() {
        let list = "C:\\file.txt\0".repeat(MAX_FILE_LIST_ITEMS);
        assert!(file_payload(&wide_dropfiles(&format!("{list}\0"))).is_ok());
        let error = file_payload(&wide_dropfiles(&format!("{list}{list}\0"))).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ContextReadError>(),
            Some(ContextReadError::TooManyItems)
        ));
    }
}
