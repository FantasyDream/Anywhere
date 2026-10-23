//! Persisted capture policy, separate from Launcher auto-fill and window retention.
use anyhow::{ensure, Result};

pub const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryPolicy {
    pub paused_until_ms: Option<i64>,
    // Zero preserves the existing quota-only policy; changing this is explicit.
    pub retention_days: u32,
}

impl HistoryPolicy {
    pub fn validate(self) -> Result<()> {
        ensure!(
            matches!(self.retention_days, 0 | 1 | 7 | 30 | 90),
            "unsupported retention period"
        );
        ensure!(
            self.paused_until_ms.is_none_or(|value| value > 0),
            "invalid pause deadline"
        );
        Ok(())
    }

    pub fn cutoff(self, now_ms: i64) -> Option<i64> {
        (self.retention_days != 0)
            .then(|| now_ms.saturating_sub(i64::from(self.retention_days) * DAY_MS))
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
