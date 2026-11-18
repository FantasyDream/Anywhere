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
    let mut kind = [0];
    reader.read_exact(&mut kind)?;
    let mut bytes = vec![0; length - 1];
    reader.read_exact(&mut bytes)?;
    let directory = kind[0] != 0;
    Ok(Some((decode_path(bytes)?, directory, length as u64 + 4)))
}

pub(in crate::features::filesystem) fn write_match_record(
    writer: &mut impl Write,
    hit: &super::RankedHit,
) -> Result<u64> {
    if hit.normalized.len() > MAX_RECORD {
        bail!("normalized name exceeds record size");
    }
    let bytes = write_record(writer, &hit.path, hit.is_directory)?;
    writer.write_all(&hit.score.to_le_bytes())?;
    writer.write_all(&(hit.normalized.len() as u32).to_le_bytes())?;
    writer.write_all(hit.normalized.as_bytes())?;
    Ok(bytes + 6 + hit.normalized.len() as u64)
}

pub(in crate::features::filesystem) fn read_match_record(
    reader: &mut impl Read,
) -> Result<Option<(super::RankedHit, u64)>> {
    let Some((path, is_directory, bytes)) = read_record(reader)? else {
        return Ok(None);
    };
    let mut metadata = [0; 6];
    reader.read_exact(&mut metadata)?;
    let score = u16::from_le_bytes(metadata[..2].try_into()?);
    let length = u32::from_le_bytes(metadata[2..].try_into()?) as usize;
    if score > 1000 || length > MAX_RECORD {
        bail!("invalid match record");
    }
    let mut normalized = vec![0; length];
    reader.read_exact(&mut normalized)?;
    Ok(Some((
        super::RankedHit {
            path,
            is_directory,
            score,
            normalized: String::from_utf8(normalized)?,
        },
        bytes + 6 + length as u64,
    )))
}
