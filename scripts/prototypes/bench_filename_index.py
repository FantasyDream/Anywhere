#!/usr/bin/env python3
"""Design experiment, not the shipping search engine.

Synthetic names only. No home scan, file content read or application launch.
Run: python3 scripts/prototypes/bench_filename_index.py --rows 200000
"""
import argparse
import heapq
import json
import math
from pathlib import Path
import resource
import sqlite3
import subprocess
import sys
import time
import unicodedata


def normalize(name):
    return unicodedata.normalize("NFC", unicodedata.normalize("NFC", name).casefold())


def grams(name):
    # Encode Unicode pairs as ASCII words: punctuation and CJK remain literal.
    return sorted({f"g{ord(a):06x}{ord(b):06x}" for a, b in zip(name, name[1:])})


def index_tokens(name, kind):
    return " ".join(grams(name) + ["type" + kind, "hidden" if name.startswith('.') else "visible"])


def connect(path, writable=False):
    db = sqlite3.connect(path if writable else f"file:{path}?mode=ro", uri=not writable)
    db.execute("PRAGMA cache_size=-4096")
    db.execute("PRAGMA mmap_size=0")
    db.execute("PRAGMA temp_store=FILE")
    db.execute("PRAGMA busy_timeout=25")
    return db


def build(path, count):
    before = peak_mib()
    path.parent.mkdir(parents=True, exist_ok=True)
    for suffix in ["", "-wal", "-shm"]:
        Path(str(path) + suffix).unlink(missing_ok=True)
    db = connect(str(path), True)
    db.executescript("""
      PRAGMA journal_mode=WAL;
      CREATE TABLE directories(id INTEGER PRIMARY KEY, path TEXT NOT NULL);
      CREATE TABLE entries(id INTEGER PRIMARY KEY, parent INTEGER NOT NULL,
        name_norm TEXT NOT NULL, type TEXT NOT NULL, hidden INTEGER NOT NULL);
      CREATE INDEX entry_names ON entries(name_norm, id);
      CREATE VIRTUAL TABLE pairs USING fts5(tokens, tokenize='ascii',
        content='', contentless_delete=1, detail=none);
    """)
    templates = [
        ("report_{:06d}.pdf", "pdf"), ("settings_controller_{:06d}.dart", "code"),
        ("季度报告_{:06d}.md", "document"), ("配置_configuration_{:06d}.toml", "code"),
        ("image_{:06d}.png", "image"), ("package_{:06d}.json", "code"),
        ("notes_{:06d}.txt", "document"), ("build_{:06d}.rs", "code"),
        (".env_{:06d}", "code"), ("archive_{:06d}.zip", "archive"),
        ("invoice_{:06d}.xlsx", "document"), ("abcabc_{:06d}.txt", "document"),
    ]
    started = time.perf_counter()
    for first in range(1, count + 1, 256):
        with db:
            for ident in range(first, min(first + 256, count + 1)):
                parent = (ident - 1) // 32 + 1
                if (ident - 1) % 32 == 0:
                    db.execute("INSERT INTO directories VALUES(?,?)", (parent, f"/fixture/project-{parent:06d}"))
                pattern, kind = templates[ident % len(templates)]
                name = normalize(pattern.format(ident))
                if ident == count - 1:
                    name = "abbax.txt" # Contains ab and ba, but not aba.
                    kind = "document"
                elif ident == count - 2:
                    name = "literal[1]*.txt"
                    kind = "document"
                elif ident == count - 3:
                    name = "café.txt"
                    kind = "document"
                db.execute("INSERT INTO entries VALUES(?,?,?,?,?)", (ident, parent, name, kind, int(name.startswith('.'))))
                db.execute("INSERT INTO pairs(rowid,tokens) VALUES(?,?)", (ident, index_tokens(name, kind)))
    # A late PDF match: filters must apply before any result cap.
    with db:
        db.execute("UPDATE entries SET name_norm='settings-late.pdf',type='pdf',hidden=0 WHERE id=?", (count,))
        db.execute("INSERT OR REPLACE INTO pairs(rowid,tokens) VALUES(?,?)", (count, index_tokens("settings-late.pdf", "pdf")))
    with db:
        db.execute("INSERT INTO pairs(pairs) VALUES('optimize')")
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    elapsed = time.perf_counter() - started
    db.close()
    return {"rows": count, "build_seconds": round(elapsed, 3),
            "db_mib": round(path.stat().st_size / 1048576, 2),
            "writer_peak_rss_mib": round(peak_mib(), 2),
            "writer_peak_increase_over_pre_build_mib": round(max(0, peak_mib() - before), 2)}


def query(db, text, kind=None, indexed=True, budget_ms=100, max_verified=4096):
    words = normalize(text).split()
    tokens = sorted({token for word in words for token in grams(word)})
    if not tokens:
        raise ValueError("experiment requires at least one two-character term")
    params = []
    if indexed:
        sql = "SELECT e.id,e.name_norm FROM pairs JOIN entries e ON e.id=pairs.rowid WHERE pairs MATCH ?"
        params.append(" AND ".join(tokens + ["visible"] + (["type" + kind] if kind else [])))
    else:
        sql = "SELECT e.id,e.name_norm FROM entries e WHERE 1=1"
    for word in words:
        sql += " AND instr(e.name_norm,?)>0"
        params.append(word)
    if kind:
        sql += " AND e.type=?"
        params.append(kind)
    sql += " AND e.hidden=0"
    start = time.perf_counter()
    deadline = start + budget_ms / 1000
    db.set_progress_handler(lambda: int(time.perf_counter() >= deadline), 1000)
    heap = []
    verified = 0
    complete = True
    cursor = None
    try:
        cursor = db.execute(sql, params)
        for ident, name in cursor:
            verified += 1
            # Simplified rank for this storage experiment, not Anywhere Ranking.
            score = (int(name.startswith(words[0])), -len(name), -ident)
            item = (score, ident)
            if len(heap) < 64:
                heapq.heappush(heap, item)
            elif score > heap[0][0]:
                heapq.heapreplace(heap, item)
            if verified >= max_verified or time.perf_counter() >= deadline:
                complete = False
                break
    except sqlite3.OperationalError as error:
        if "interrupt" not in str(error):
            raise
        complete = False
    finally:
        if cursor is not None:
            cursor.close()
        db.set_progress_handler(None, 0)
    return {"ms": (time.perf_counter() - start) * 1000,
            "returned": len(heap), "verified": verified,
            "complete_within_index": complete,
            "ids": [ident for _, ident in sorted(heap, reverse=True)]}


def peak_mib():
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return peak / (1048576 if sys.platform == "darwin" else 1024)


def probe(path, indexed):
    db = connect(str(path))
    baseline = peak_mib()
    cases = [("settings", None), ("settings", "pdf"), ("port", "pdf"),
             ("配置", None), ("季度", None), ("rs", "code"),
             ("zznomatch", None), ("aba", None)]
    report = []
    for text, kind in cases:
        measurements = [query(db, text, kind, indexed) for _ in range(21)]
        warm = sorted(item["ms"] for item in measurements[1:])
        first = measurements[0]
        report.append({"query": text, "type": kind,
                       "first_connection_ms": round(first["ms"], 3),
                       "warm_p50_ms": round(warm[len(warm) // 2], 3),
                       "warm_p95_ms": round(warm[math.ceil(len(warm) * .95) - 1], 3),
                       "returned": first["returned"], "verified": first["verified"],
                       "complete_within_index": first["complete_within_index"]})
    # Precise, late match survives filtering; reordered grams cannot invent matches.
    target = db.execute("SELECT max(id) FROM entries").fetchone()[0]
    filtered = query(db, "settings", "pdf", indexed, budget_ms=1000)
    assert filtered["ids"] == [target] and filtered["complete_within_index"]
    assert query(db, "aba", indexed=indexed, budget_ms=1000)["returned"] == 0
    assert query(db, "[1]*", indexed=indexed, budget_ms=1000)["ids"] == [target - 2]
    assert query(db, "cafe\u0301", indexed=indexed, budget_ms=1000)["ids"] == [target - 3]
    # The two-character query finds infix, not only prefixes.
    assert query(db, "rs", "code", indexed)["returned"] > 0
    peak = peak_mib()
    db.close()
    return {"strategy": "bigram_fts" if indexed else "linear_sql",
            "sqlite_version": sqlite3.sqlite_version,
            "peak_rss_mib": round(peak, 2),
            "peak_increase_over_pre_query_mib": round(max(0, peak - baseline), 2),
            "cases": report}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows", type=int, default=200000)
    parser.add_argument("--directory", type=Path, default=Path("build/file-search-design"))
    parser.add_argument("--probe", choices=["indexed", "linear"])
    args = parser.parse_args()
    if args.rows < 32:
        parser.error("--rows must be at least 32")
    path = args.directory.resolve() / "catalog.db"
    if args.probe:
        print(json.dumps(probe(path, args.probe == "indexed"), ensure_ascii=False))
        return
    report = {"dataset": build(path, args.rows), "synthetic": True,
              "os_cache_cleared": False, "runtime": "Python storage prototype; not Rust/GUI measurements"}
    for mode in ["indexed", "linear"]:
        result = subprocess.run([sys.executable, __file__, "--directory", str(args.directory), "--probe", mode],
                                capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(result.stderr)
        report[mode] = json.loads(result.stdout)
    output = args.directory / "report.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
