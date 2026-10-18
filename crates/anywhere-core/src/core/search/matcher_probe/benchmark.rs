use super::*;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

#[cfg(test)]
fn corpus(kind: &str, count: usize) -> Vec<SearchFields> {
    if kind == "actions" {
        let registry = crate::application::builtin_registry();
        let fields: Vec<_> = registry.commands().map(|c| c.search().clone()).collect();
        (0..count)
            .map(|i| fields[i % fields.len()].clone())
            .collect()
    } else {
        let apps = [
            ("Google Chrome", "chrome"),
            ("Visual Studio Code", "vscode"),
            ("IntelliJ IDEA", "idea"),
            ("Microsoft Edge", "edge"),
            ("AB Download Manager", "abdownload"),
            ("飞书", "feishu"),
            ("微信", "wechat"),
            ("访达", "finder"),
            ("Terminal", "terminal"),
            ("Café Editor", "cafe"),
            ("工具箱", "toolbox"),
            ("Node Package Manager", "npm"),
        ];
        (0..count)
            .map(|i| {
                let (title, alias) = apps[i % apps.len()];
                SearchFields::new(
                    &format!("{title} {i}"),
                    &[alias.into()],
                    &[format!("vendor.application{i}")],
                )
            })
            .collect()
    }
}

fn summary(levels: impl Iterator<Item = Option<MatchLevel>>) -> (usize, u64) {
    levels
        .enumerate()
        .fold((0, 0), |(count, sum), (i, level)| match level {
            Some(level) => (
                count + 1,
                sum.wrapping_add((i as u64 + 1) * level.score(true) as u64),
            ),
            None => (count, sum),
        })
}

#[test]
#[ignore = "release matcher resource benchmark; run via scripts/bench-matcher.py"]
fn compare() {
    let backend = std::env::var("ANYWHERE_MATCHER_BACKEND").expect("explicit backend");
    if backend == "baseline" {
        println!("MATCHER_METRICS {{\"backend\":\"baseline\"}}");
        return;
    }
    let mode = match backend.as_str() {
        "existing" | "legacy" | "prepared" => Mode::Native,
        "nucleo" => Mode::Substring,
        "fuzzy" => Mode::Fuzzy,
        _ => panic!("unknown backend"),
    };
    let kind = std::env::var("ANYWHERE_MATCHER_CORPUS").unwrap();
    assert!(kind == "apps" || kind == "actions");
    let count: usize = std::env::var("ANYWHERE_MATCHER_ROWS")
        .unwrap()
        .parse()
        .unwrap();
    let fields = corpus(&kind, count);
    let started = Instant::now();
    let prepared: Vec<_> = if backend == "existing" || backend == "legacy" {
        Vec::new()
    } else {
        fields.iter().map(|f| Prepared::new(f, mode)).collect()
    };
    let mut matcher = if mode == Mode::Native {
        None
    } else {
        matcher()
    };
    let prepare_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut queries = Vec::new();
    for raw in [
        "chrome",
        "chr",
        "hrome",
        "co",
        "gchr",
        "vsc",
        "git sta",
        "copy branch",
        "brnch",
        "复制 名称",
        "google 不存在",
        "app missing",
    ] {
        let mut times = Vec::new();
        let mut result = None;
        for sample in 0..230 {
            let started = Instant::now();
            let query = Query::parse(black_box(raw));
            let actual = if backend == "legacy" {
                summary(fields.iter().map(|f| legacy_matched(f, &query)))
            } else if backend == "existing" {
                summary(fields.iter().map(|f| f.matched(&query)))
            } else {
                let tokens = tokens(&query, mode);
                summary(
                    prepared
                        .iter()
                        .map(|p| p.matched(&query, &tokens, &mut matcher, mode)),
                )
            };
            let elapsed = started.elapsed().as_nanos();
            if let Some(previous) = result {
                assert_eq!(actual, previous);
            }
            result = Some(black_box(actual));
            if sample >= 30 {
                times.push(elapsed);
            }
        }
        times.sort_unstable();
        let (matches, checksum) = result.unwrap();
        // Check complete membership and tiers, outside the timed section.
        let query = Query::parse(raw);
        let tokens = tokens(&query, mode);
        let mut digest = Sha256::new();
        for (i, fields) in fields.iter().enumerate() {
            let level = if backend == "legacy" {
                legacy_matched(fields, &query)
            } else if backend == "existing" {
                fields.matched(&query)
            } else {
                prepared[i].matched(&query, &tokens, &mut matcher, mode)
            };
            if let Some(level) = level {
                digest.update((i as u64).to_le_bytes());
                digest.update(level.score(true).to_le_bytes());
            }
        }
        queries.push(
            serde_json::json!({"query":raw,"p50_us":times[100] as f64/1000.0,
            "p95_us":times[190] as f64/1000.0,"matches":matches,"level_checksum":checksum,
            "candidate_tiers_sha256":format!("{:x}",digest.finalize())}),
        );
    }
    println!(
        "MATCHER_METRICS {}",
        serde_json::json!({"backend":backend,"corpus":kind,"rows":count,"prepare_ms":prepare_ms,"queries":queries})
    );
}
