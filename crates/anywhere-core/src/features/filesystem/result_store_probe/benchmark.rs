use super::{append, sqlite::SqliteResults, store};
use anywhere_platform::search_storage::encode_path;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::Instant,
};

fn disk_bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum()
}

fn path(i: usize) -> PathBuf {
    // The same raw identities in every process; no fixture directory names in hashes.
    PathBuf::from(format!(
        "/probe/project-{}/nested directory/report-{i:07}.txt",
        i % 79
    ))
}

#[test]
#[ignore = "release resource benchmark; run via scripts/bench-result-store.py"]
fn compare() {
    let backend = std::env::var("ANYWHERE_PROBE_BACKEND").expect("explicit backend required");
    if backend == "baseline" {
        println!("RESULT_STORE_METRICS {{\"backend\":\"baseline\"}}");
        return;
    }
    assert!(backend == "external" || backend == "sqlite");
    let count: usize = std::env::var("ANYWHERE_PROBE_ROWS")
        .unwrap()
        .parse()
        .unwrap();
    let waves: usize = std::env::var("ANYWHERE_PROBE_WAVES")
        .unwrap()
        .parse()
        .unwrap();
    assert!(count >= 100 && waves > 0 && waves <= count);
    let store = store();
    if std::env::var("ANYWHERE_PROBE_CANCEL").as_deref() == Ok("1") {
        append(&store, (0..count).rev().map(|i| (path(i), false)));
        let checks = AtomicUsize::new(0);
        let requested = Mutex::new(None);
        let cancel = || {
            if checks.fetch_add(1, Ordering::Relaxed) >= 600 {
                requested.lock().unwrap().get_or_insert_with(Instant::now);
                true
            } else {
                false
            }
        };
        let mut sqlite = if backend == "sqlite" {
            Some(SqliteResults::new(&store).unwrap())
        } else {
            None
        };
        let error = if let Some(sqlite) = &mut sqlite {
            sqlite.more(&store, &HashSet::new(), &cancel).unwrap_err()
        } else {
            store
                .more(&HashSet::new(), &cancel)
                .err()
                .expect("cancelled")
        };
        assert!(error.to_string().contains("cancelled"), "{error}");
        let elapsed = requested.lock().unwrap().unwrap().elapsed();
        println!(
            "RESULT_STORE_METRICS {}",
            serde_json::json!({
                "backend":backend,"rows":count,"cancelled":true,
                "cancel_return_ms":elapsed.as_secs_f64()*1000.0
            })
        );
        return;
    }
    let mut displayed = HashSet::new();
    let mut digest = Sha256::new();
    // Preserve the same already displayed prefix, as with a provisional scan batch.
    for i in 0..7 {
        let p = path(i);
        digest.update(encode_path(&p));
        displayed.insert(p);
    }
    let mut sqlite = None;
    let mut backend_ms = 0.0;
    let mut first_ms = 0.0;
    let mut slowest_ms: f64 = 0.0;
    let mut peak_disk = 0;
    let mut batches = 0;
    for wave in 0..waves {
        append(
            &store,
            (count * wave / waves..count * (wave + 1) / waves)
                .rev()
                .map(|i| (path(i), false)),
        );
        let started = Instant::now();
        if backend == "sqlite" && sqlite.is_none() {
            sqlite = Some(SqliteResults::new(&store).unwrap());
        }
        loop {
            let next_started = if batches == 0 {
                started
            } else {
                Instant::now()
            };
            let hits = if let Some(sqlite) = &mut sqlite {
                sqlite.more(&store, &displayed, &|| false).unwrap()
            } else {
                store.more(&displayed, &|| false).unwrap().hits
            };
            let elapsed = next_started.elapsed().as_secs_f64() * 1000.0;
            backend_ms += elapsed;
            slowest_ms = slowest_ms.max(elapsed);
            if batches == 0 {
                first_ms = elapsed;
            }
            batches += 1;
            peak_disk = peak_disk.max(disk_bytes(store.workspace()));
            let empty = hits.is_empty();
            for hit in hits {
                digest.update(encode_path(&hit.path));
                assert!(displayed.insert(hit.path));
            }
            // Leave an unread tail until the final wave, exercising incremental merges.
            if empty || wave + 1 < waves {
                break;
            }
        }
    }
    assert_eq!(displayed.len(), count, "omitted records");
    println!(
        "RESULT_STORE_METRICS {}",
        serde_json::json!({
            "backend":backend,"rows":count,"waves":waves,"returned":displayed.len(),"batches":batches,
            "first_batch_ms":first_ms,"all_batches_ms":backend_ms,"slowest_batch_ms":slowest_ms,
            "temporary_bytes_sampled":peak_disk,"displayed_order_sha256":format!("{:x}",digest.finalize())
        })
    );
}
