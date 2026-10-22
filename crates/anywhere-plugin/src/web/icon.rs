use anyhow::{ensure, Context, Result};
use std::io::Cursor;

pub(super) const MAX_BYTES: u64 = 256 * 1024;
const MAX_DIMENSION: u32 = 512;
const DECODE_BUDGET: usize = 2 * 1024 * 1024;

/// Decode only the declared launcher icon, with bounds checked before allocating pixels.
pub(super) fn validate(bytes: &[u8]) -> Result<()> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: DECODE_BUDGET,
    });
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info().context("invalid Web icon PNG")?;
    let info = reader.info();
    ensure!(
        (1..=MAX_DIMENSION).contains(&info.width) && (1..=MAX_DIMENSION).contains(&info.height),
        "Web icon must be at most 512 by 512 pixels"
    );
    ensure!(
        info.animation_control.is_none(),
        "animated Web icon unsupported"
    );
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= DECODE_BUDGET)
        .context("Web icon exceeds decode budget")?;
    reader
        .next_frame(&mut vec![0; size])
        .context("invalid Web icon pixels")?;
    reader.finish().context("invalid Web icon trailer")?;
    Ok(())
}
