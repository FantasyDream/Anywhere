//! Length-delimited raw platform paths for query-local records and sort runs.
use anyhow::{bail, Result};
use anywhere_platform::search_storage::{decode_path, encode_path};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) const MAX_RECORD: usize = 256 * 1024;

pub(in crate::features::filesystem) fn write_record(
    writer: &mut impl Write,
    path: &Path,
    directory: bool,
) -> Result<u64> {
    let bytes = encode_path(path);
    if bytes.len() > MAX_RECORD {
        bail!("path exceeds record size");
    }
    writer.write_all(&(bytes.len() as u32 + 1).to_le_bytes())?;
    writer.write_all(&[u8::from(directory)])?;
    writer.write_all(&bytes)?;
    Ok(bytes.len() as u64 + 5)
}
pub(in crate::features::filesystem) fn read_record(
    reader: &mut impl Read,
) -> Result<Option<(PathBuf, bool, u64)>> {
    let mut length = [0; 4];
    if reader.read(&mut length[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut length[1..])?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_RECORD + 1 {
        bail!("invalid path record length");
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    let directory = bytes.remove(0) != 0;
    Ok(Some((decode_path(bytes)?, directory, length as u64 + 4)))
}
