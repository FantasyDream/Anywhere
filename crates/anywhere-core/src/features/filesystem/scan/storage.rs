//! A query owns one shared byte budget across traversal and merge files.
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

pub(super) const MAX_STORAGE_BYTES: u64 = 512 * 1024 * 1024;

pub(in crate::features::filesystem) struct BudgetWriter<W> {
    writer: W,
    bytes: Arc<AtomicU64>,
}
impl<W> BudgetWriter<W> {
    pub(super) fn new(writer: W, bytes: Arc<AtomicU64>) -> Self {
        Self { writer, bytes }
    }
}
impl<W: Write> Write for BudgetWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes.len() as u64)
                    .filter(|next| *next <= MAX_STORAGE_BYTES)
            })
            .map_err(|_| io::Error::other("temporary search storage exceeded 512 MiB"))?;
        let result = self.writer.write(bytes);
        let committed = result.as_ref().copied().unwrap_or(0);
        self.bytes
            .fetch_sub((bytes.len() - committed) as u64, Ordering::AcqRel);
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_budget_rejects_before_writing_and_refunds_failed_writes() {
        let budget = Arc::new(AtomicU64::new(MAX_STORAGE_BYTES - 4));
        let mut writer = BudgetWriter::new(Vec::<u8>::new(), budget.clone());
        assert!(writer.write_all(b"large").is_err());
        assert!(writer.writer.is_empty());
        writer.write_all(b"fits").unwrap();
        assert_eq!(budget.load(Ordering::Acquire), MAX_STORAGE_BYTES);
        assert!(writer.write_all(b"x").is_err());
        struct Fails;
        impl Write for Fails {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("write failed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let budget = Arc::new(AtomicU64::new(0));
        assert!(BudgetWriter::new(Fails, budget.clone())
            .write_all(b"fail")
            .is_err());
        assert_eq!(budget.load(Ordering::Acquire), 0);
    }
}
