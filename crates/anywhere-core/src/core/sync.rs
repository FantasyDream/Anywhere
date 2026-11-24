//! Explicit mutex and read-write lock poison policies.
use anyhow::{anyhow, Result};
use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub fn lock_or_deny<'a, T>(mutex: &'a Mutex<T>, name: &'static str) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| {
        tracing::error!(lock = name, "lock_poisoned_denied");
        anyhow!("{name} unavailable")
    })
}

pub fn lock_state<'a, T>(mutex: &'a Mutex<T>, name: &'static str) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| {
        tracing::error!(lock = name, "service_state_lock_poisoned");
        anyhow!("{name} unavailable")
    })
}

pub fn lock_for_revoke<'a, T>(mutex: &'a Mutex<T>, name: &'static str) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        tracing::error!(lock = name, "revoke_lock_poisoned_recovered");
        poisoned.into_inner()
    })
}

pub fn lock_cache<'a, T: Default>(mutex: &'a Mutex<T>, name: &'static str) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        tracing::warn!(lock = name, "cache_lock_poisoned_reset");
        let mut guard = poisoned.into_inner();
        *guard = T::default();
        mutex.clear_poison();
        guard
    })
}

pub fn read_or_deny<'a, T>(
    lock: &'a RwLock<T>,
    name: &'static str,
) -> Result<RwLockReadGuard<'a, T>> {
    lock.read().map_err(|_| {
        tracing::error!(lock = name, "rwlock_read_poisoned_denied");
        anyhow!("{name} unavailable")
    })
}

pub fn write_or_deny<'a, T>(
    lock: &'a RwLock<T>,
    name: &'static str,
) -> Result<RwLockWriteGuard<'a, T>> {
    lock.write().map_err(|_| {
        tracing::error!(lock = name, "rwlock_write_poisoned_denied");
        anyhow!("{name} unavailable")
    })
}

pub fn read_state<'a, T>(
    lock: &'a RwLock<T>,
    name: &'static str,
) -> Result<RwLockReadGuard<'a, T>> {
    lock.read().map_err(|_| {
        tracing::error!(lock = name, "service_state_rwlock_read_poisoned");
        anyhow!("{name} unavailable")
    })
}

pub fn write_state<'a, T>(
    lock: &'a RwLock<T>,
    name: &'static str,
) -> Result<RwLockWriteGuard<'a, T>> {
    lock.write().map_err(|_| {
        tracing::error!(lock = name, "service_state_rwlock_write_poisoned");
        anyhow!("{name} unavailable")
    })
}

pub fn write_for_revoke<'a, T>(lock: &'a RwLock<T>, name: &'static str) -> RwLockWriteGuard<'a, T> {
    lock.write().unwrap_or_else(|poisoned| {
        tracing::error!(lock = name, "revoke_rwlock_poisoned_recovered");
        poisoned.into_inner()
    })
}

pub fn read_cache<'a, T>(
    lock: &'a RwLock<T>,
    name: &'static str,
) -> Option<RwLockReadGuard<'a, T>> {
    match lock.read() {
        Ok(guard) => Some(guard),
        Err(_) => {
            tracing::warn!(lock = name, "cache_rwlock_read_poisoned");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poison_mutex<T: Send + 'static>(mutex: &'static Mutex<T>) {
        let _ = std::thread::spawn(move || {
            let _guard = mutex.lock().unwrap();
            panic!("poison mutex");
        })
        .join();
    }

    fn poison_rwlock<T: Send + Sync + 'static>(lock: &'static RwLock<T>) {
        let _ = std::thread::spawn(move || {
            let _guard = lock.write().unwrap();
            panic!("poison rwlock");
        })
        .join();
    }

    #[test]
    fn mutex_helpers_enforce_deny_revoke_and_cache_reset_on_poison() {
        let deny_mutex: &'static Mutex<i32> = Box::leak(Box::new(Mutex::new(42)));
        poison_mutex(deny_mutex);
        assert!(lock_or_deny(deny_mutex, "deny").is_err());
        assert!(lock_state(deny_mutex, "state").is_err());

        let revoke_mutex: &'static Mutex<bool> = Box::leak(Box::new(Mutex::new(false)));
        poison_mutex(revoke_mutex);
        {
            let mut guard = lock_for_revoke(revoke_mutex, "revoke");
            *guard = true;
        }
        assert!(*lock_for_revoke(revoke_mutex, "revoke"));

        let cache_mutex: &'static Mutex<Vec<i32>> = Box::leak(Box::new(Mutex::new(vec![1, 2, 3])));
        poison_mutex(cache_mutex);
        assert!(lock_cache(cache_mutex, "cache").is_empty());
        lock_cache(cache_mutex, "cache").push(4);
        assert_eq!(*lock_cache(cache_mutex, "cache"), vec![4]);
    }

    #[test]
    fn rwlock_helpers_enforce_deny_revoke_and_cache_on_poison() {
        let rw: &'static RwLock<Vec<i32>> = Box::leak(Box::new(RwLock::new(vec![9])));
        poison_rwlock(rw);
        assert!(read_or_deny(rw, "rw").is_err());
        assert!(write_or_deny(rw, "rw").is_err());
        assert!(read_state(rw, "rw").is_err());
        assert!(write_state(rw, "rw").is_err());
        assert!(read_cache(rw, "rw").is_none());
        {
            let mut guard = write_for_revoke(rw, "rw");
            guard.clear();
        }
        assert!(write_for_revoke(rw, "rw").is_empty());
    }
}
