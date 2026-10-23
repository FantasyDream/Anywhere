//! Clipboard history contract. Bodies never enter launcher contexts or usage.
use anyhow::Result;
use anywhere_platform::clipboard::ClipboardPayload;
pub const MAX_ENTRY_BYTES: usize = anywhere_platform::clipboard::MAX_PAYLOAD_BYTES;
pub const MAX_ENTRIES: usize = 1000;
pub const MAX_STORED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_QUERY_BYTES: usize = 1024;
pub const SUMMARY_CHARACTERS: usize = 120;
pub const MAX_PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryItem {
    pub id: i64,
    pub summary: String,
    pub kind: String,
    pub copied_at_ms: i64,
    pub byte_count: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryDetail {
    pub item: HistoryItem,
    pub text: String,
    pub image_png: Vec<u8>,
    pub files: Vec<String>,
    pub formats: Vec<String>,
}
pub trait HistoryRepository: Send {
    fn policy(&self) -> Result<super::policy::HistoryPolicy>;
    fn set_policy(&mut self, policy: super::policy::HistoryPolicy) -> Result<()>;
    fn expire(&mut self, cutoff_ms: i64) -> Result<()>;
    /// Entry count and logical stored bytes; never loads clipboard bodies.
    fn usage(&self) -> Result<(u32, u32)>;
    fn recording_enabled(&self) -> Result<bool>;
    fn set_recording_enabled(&mut self, enabled: bool) -> Result<()>;
    fn record_payload(
        &mut self,
        payload: &ClipboardPayload,
        copied_at_ms: i64,
    ) -> Result<Option<i64>>;
    fn record(&mut self, text: &str, copied_at_ms: i64) -> Result<Option<i64>> {
        self.record_payload(&ClipboardPayload::text(text), copied_at_ms)
    }
    fn search(&self, query: &str) -> Result<Vec<HistoryItem>>;
    fn item(&self, id: i64) -> Result<Option<HistoryItem>>;
    fn payload(&self, id: i64) -> Result<Option<ClipboardPayload>>;
    fn delete(&mut self, id: i64) -> Result<()>;
    fn clear(&mut self) -> Result<()>;
}
