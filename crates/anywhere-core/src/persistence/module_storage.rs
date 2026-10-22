//! SQLite adapter for bounded, namespace-scoped extension storage.
use crate::core::storage::*;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ModuleStorageHost {
    connection: Arc<Mutex<Connection>>,
    path: Option<PathBuf>,
}
impl ModuleStorageHost {
    pub fn open(path: &Path) -> Result<Self> {
        let mut host = Self::connection(Connection::open(path)?)?;
        host.path = Some(path.into());
        Ok(host)
    }
    pub fn memory() -> Result<Self> {
        Self::connection(Connection::open_in_memory()?)
    }
    fn connection(connection: Connection) -> Result<Self> {
        Self::configure(&connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            path: None,
        })
    }
    fn configure(connection: &Connection) -> Result<()> {
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        ensure!(version <= 1, "unsupported module storage version");
        connection.execute_batch("PRAGMA cache_size=-256; CREATE TABLE IF NOT EXISTS extension_values (namespace TEXT NOT NULL, key TEXT NOT NULL, value BLOB NOT NULL, PRIMARY KEY(namespace,key)); PRAGMA user_version=1;")?;
        Ok(())
    }
    /// Trusted Host cleanup only, never exported to a module or browser.
    pub fn clear_namespace(&self, namespace: &str) -> Result<()> {
        validate(namespace)?;
        self.connection
            .lock()
            .map_err(|_| anyhow::anyhow!("module storage unavailable"))?
            .execute(
                "DELETE FROM extension_values WHERE namespace=?1",
                params![namespace],
            )?;
        Ok(())
    }
    /// Explicit data refresh also follows a repaired/replaced database file. Bound handles
    /// share the slot, so no old action can keep writing through a detached connection.
    pub fn reopen(&self) -> Result<()> {
        if let Some(path) = &self.path {
            let mut slot = self
                .connection
                .lock()
                .map_err(|_| anyhow::anyhow!("module storage unavailable"))?;
            let replacement = Connection::open(path)?;
            Self::configure(&replacement)?;
            *slot = replacement;
        }
        Ok(())
    }
    /// Only the host holds this factory. Modules receive the resulting bound handle.
    pub fn bind(&self, namespace: &str) -> Result<Arc<dyn ExtensionStorage>> {
        self.bind_with_quota(namespace, MAX_NAMESPACE_BYTES, MAX_NAMESPACE_KEYS)
    }
    /// Trusted Host chooses the quota. Neither modules nor Web messages receive this factory.
    pub fn bind_with_quota(
        &self,
        namespace: &str,
        max_bytes: usize,
        max_keys: usize,
    ) -> Result<Arc<dyn ExtensionStorage>> {
        validate(namespace)?;
        ensure!(
            max_bytes > 0 && max_bytes <= 1024 * 1024 && max_keys > 0 && max_keys <= 256,
            "invalid storage quota"
        );
        Ok(Arc::new(ScopedStorage {
            host: self.clone(),
            namespace: namespace.into(),
            max_bytes,
            max_keys,
        }))
    }
}
struct ScopedStorage {
    host: ModuleStorageHost,
    namespace: String,
    max_bytes: usize,
    max_keys: usize,
}
fn validate(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty()
            && key.len() <= MAX_KEY_BYTES
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
        "invalid module storage key"
    );
    Ok(())
}
impl ExtensionStorage for ScopedStorage {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        validate(key)?;
        let connection = self
            .host
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("module storage unavailable"))?;
        read_value(&connection, &self.namespace, key)
    }
    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        ensure!(value.len() <= MAX_VALUE_BYTES, StorageQuotaExceeded);
        self.update(key, &mut |_| Ok(Some(value.to_vec())))?;
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<()> {
        validate(key)?;
        self.host
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("module storage unavailable"))?
            .execute(
                "DELETE FROM extension_values WHERE namespace=?1 AND key=?2",
                params![self.namespace, key],
            )?;
        Ok(())
    }
    fn update(&self, key: &str, change: &mut StorageUpdate<'_>) -> Result<Option<Vec<u8>>> {
        validate(key)?;
        let mut connection = self
            .host
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("module storage unavailable"))?;
        // Reserve the write before reading; deferred transactions can race other connections.
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = read_value(&transaction, &self.namespace, key)?;
        let value = change(previous.as_deref())?;
        if let Some(value) = &value {
            ensure!(value.len() <= MAX_VALUE_BYTES, StorageQuotaExceeded);
            let (bytes, keys): (usize, usize) = transaction.query_row("SELECT COALESCE(SUM(length(value)),0),count(*) FROM extension_values WHERE namespace=?1 AND key!=?2", params![self.namespace,key], |r| Ok((r.get(0)?,r.get(1)?)))?;
            ensure!(
                bytes + value.len() <= self.max_bytes && keys < self.max_keys,
                StorageQuotaExceeded
            );
            transaction.execute("INSERT INTO extension_values(namespace,key,value) VALUES(?1,?2,?3) ON CONFLICT(namespace,key) DO UPDATE SET value=excluded.value", params![self.namespace,key,value])?;
        } else {
            transaction.execute(
                "DELETE FROM extension_values WHERE namespace=?1 AND key=?2",
                params![self.namespace, key],
            )?;
        }
        transaction.commit()?;
        Ok(value)
    }
}
fn read_value(connection: &Connection, namespace: &str, key: &str) -> Result<Option<Vec<u8>>> {
    let row: Option<(usize, Option<Vec<u8>>)> = connection
            .query_row(
                "SELECT length(value), CASE WHEN length(value)<=?3 THEN value ELSE NULL END FROM extension_values WHERE namespace=?1 AND key=?2",
                params![namespace, key, MAX_VALUE_BYTES],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
    ensure!(
        row.as_ref()
            .is_none_or(|(length, _)| *length <= MAX_VALUE_BYTES),
        "stored value exceeds limit"
    );
    Ok(row.and_then(|(_, value)| value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_connections_serialize_updates_and_failure_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("modules.sqlite3");
        let a = ModuleStorageHost::open(&path)
            .unwrap()
            .bind("counter")
            .unwrap();
        let b = ModuleStorageHost::open(&path)
            .unwrap()
            .bind("counter")
            .unwrap();
        let threads: Vec<_> = [a.clone(), b]
            .into_iter()
            .map(|storage| {
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        storage
                            .update("value", &mut |raw| {
                                let n =
                                    raw.map_or(0, |v| u32::from_le_bytes(v.try_into().unwrap()));
                                Ok(Some((n + 1).to_le_bytes().to_vec()))
                            })
                            .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(a.get("value").unwrap().unwrap(), 100u32.to_le_bytes());
        assert!(a
            .update("value", &mut |_| anyhow::bail!("mutation failed"))
            .is_err());
        assert!(a
            .update("value", &mut |_| Ok(Some(vec![0; MAX_VALUE_BYTES + 1])))
            .is_err());
        assert_eq!(a.get("value").unwrap().unwrap(), 100u32.to_le_bytes());
    }
    #[test]
    fn namespaces_restart_validation_and_failed_quota_preserve_values() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("modules.sqlite3");
        let host = ModuleStorageHost::open(&path).unwrap();
        let a = host.bind("a").unwrap();
        let b = host.bind("b").unwrap();
        a.put("shared", b"A").unwrap();
        b.put("shared", b"B").unwrap();
        assert_eq!(a.get("shared").unwrap().unwrap(), b"A");
        assert!(a.put("../b/shared", b"bad").is_err());
        assert!(host.bind("../b").is_err());
        assert!(a.put("shared", &vec![0; MAX_VALUE_BYTES + 1]).is_err());
        assert_eq!(a.get("shared").unwrap().unwrap(), b"A");
        for i in 0..3 {
            a.put(&format!("large{i}"), &vec![0; MAX_VALUE_BYTES])
                .unwrap();
        }
        assert!(a.put("shared", &vec![0; MAX_VALUE_BYTES]).is_ok());
        assert!(a.put("overflow", b"x").is_err());
        assert!(a.get("overflow").unwrap().is_none());
        b.remove("shared").unwrap();
        assert!(b.get("shared").unwrap().is_none());
        let restarted = ModuleStorageHost::open(&path).unwrap();
        assert_eq!(
            restarted
                .bind("a")
                .unwrap()
                .get("shared")
                .unwrap()
                .unwrap()
                .len(),
            MAX_VALUE_BYTES
        );
        assert!(restarted
            .bind("b")
            .unwrap()
            .get("shared")
            .unwrap()
            .is_none());
    }
    #[test]
    fn key_count_is_bounded_and_existing_keys_can_be_updated() {
        let store = ModuleStorageHost::memory()
            .unwrap()
            .bind("bounded")
            .unwrap();
        for i in 0..MAX_NAMESPACE_KEYS {
            store.put(&format!("k{i}"), b"x").unwrap();
        }
        assert!(store.put("overflow", b"x").is_err());
        store.put("k0", b"updated").unwrap();
        store.remove("k1").unwrap();
        store.put("replacement", b"x").unwrap();
    }
}
