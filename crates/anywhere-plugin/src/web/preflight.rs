//! Bound central-directory allocation BEFORE zip-rs constructs its metadata map.
//! This is a restricted package envelope, not a decompressor or general ZIP parser.
use super::MAX_ENTRIES;
use anyhow::{ensure, Context, Result};

pub(super) fn check(bytes: &[u8]) -> Result<usize> {
    let end = bytes.len().checked_sub(22).context("truncated Web ZIP")?;
    ensure!(
        bytes.starts_with(b"PK\x03\x04"),
        "Web ZIP must start with a local header"
    );
    ensure!(
        bytes.get(end..end + 4) == Some(b"PK\x05\x06"),
        "expected single-disk ZIP without archive comment or ZIP64"
    );
    ensure!(
        u16_at(bytes, end + 4)? == 0
            && u16_at(bytes, end + 6)? == 0
            && u16_at(bytes, end + 20)? == 0,
        "unsupported Web ZIP envelope"
    );
    let count = u16_at(bytes, end + 10)? as usize;
    ensure!(
        count > 0 && count <= MAX_ENTRIES && u16_at(bytes, end + 8)? as usize == count,
        "Web ZIP entry count exceeds limit or is inconsistent"
    );
    let length = u32_at(bytes, end + 12)? as usize;
    let start = u32_at(bytes, end + 16)? as usize;
    ensure!(
        length <= 4 * 1024 * 1024 && start.checked_add(length) == Some(end),
        "Web ZIP directory exceeds limit or is inconsistent"
    );
    let mut cursor = start;
    for _ in 0..count {
        ensure!(
            cursor.checked_add(46).is_some_and(|next| next <= end)
                && bytes.get(cursor..cursor + 4) == Some(b"PK\x01\x02"),
            "invalid Web ZIP directory"
        );
        let name = u16_at(bytes, cursor + 28)? as usize;
        let extra = u16_at(bytes, cursor + 30)? as usize;
        let comment = u16_at(bytes, cursor + 32)? as usize;
        ensure!(
            name > 0 && name <= 1025 && extra <= 4096 && comment <= 1024,
            "Web ZIP metadata exceeds limit"
        );
        ensure!(
            u16_at(bytes, cursor + 34)? == 0,
            "multi-disk Web ZIP is unsupported"
        );
        let extra_start = cursor + 46 + name;
        let next = extra_start + extra + comment;
        ensure!(next <= end, "truncated Web ZIP metadata");
        // Ignore only inert timestamps/UIDs, never interpret link or executable metadata.
        validate_extra(&bytes[extra_start..extra_start + extra])?;
        let local = u32_at(bytes, cursor + 42)? as usize;
        ensure!(
            local.checked_add(30).is_some_and(|next| next <= start)
                && bytes.get(local..local + 4) == Some(b"PK\x03\x04"),
            "invalid Web ZIP local header"
        );
        ensure!(
            u32_at(bytes, local + 18)? != u32::MAX && u32_at(bytes, local + 22)? != u32::MAX,
            "ZIP64 Web resources are unsupported"
        );
        let local_name = u16_at(bytes, local + 26)? as usize;
        let local_extra = u16_at(bytes, local + 28)? as usize;
        ensure!(
            local_name > 0
                && local_name <= 1025
                && local_extra <= 4096
                && local + 30 + local_name + local_extra <= start,
            "Web local metadata exceeds limit"
        );
        let local_extra_start = local + 30 + local_name;
        validate_extra(&bytes[local_extra_start..local_extra_start + local_extra])?;
        cursor = next;
    }
    ensure!(
        cursor == end,
        "Web ZIP directory count does not match contents"
    );
    Ok(count)
}

fn validate_extra(bytes: &[u8]) -> Result<()> {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let kind = u16_at(bytes, cursor)?;
        let size = u16_at(bytes, cursor + 2)? as usize;
        ensure!(
            matches!(kind, 0x5455 | 0x7875 | 0x000a),
            "unsupported Web ZIP extra field"
        );
        cursor += 4 + size;
        ensure!(cursor <= bytes.len(), "truncated Web ZIP extra field");
    }
    Ok(())
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    let value = bytes.get(at..at + 2).context("truncated Web ZIP field")?;
    Ok(u16::from_le_bytes(value.try_into()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let value = bytes.get(at..at + 4).context("truncated Web ZIP field")?;
    Ok(u32::from_le_bytes(value.try_into()?))
}
