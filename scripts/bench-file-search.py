#!/usr/bin/env python3
"""Bounded synthetic filename traversal probe; never opens the GUI or clears OS caches.

Build: cargo build --release -p anywhere-core --example file_search_measure --locked
Run on macOS: python3 scripts/bench-file-search.py --output docs/benchmarks/file-search-v2.json
RSS covers the standalone Rust process, not Spotlight or Flutter.
"""
import argparse
import json
import platform
import re
import subprocess
import tempfile
from pathlib import Path


def measure(binary, args):
    process = subprocess.run(['/usr/bin/time', '-l', str(binary), *args],
                             check=True, text=True, capture_output=True)
    result = json.loads(process.stdout)
    match = re.search(r'(\d+)\s+maximum resident set size', process.stderr)
    result['max_rss_bytes'] = int(match[1]) if match else None
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path,
                        default=Path('target/release/examples/file_search_measure'))
    parser.add_argument('--output', type=Path, required=True)
    options = parser.parse_args()
    if platform.system() != 'Darwin':
        parser.error('this RSS sampler uses macOS /usr/bin/time -l')
    binary = options.binary.resolve()
    result = {'platform': platform.platform(), 'probe': 'release Rust walker + pagination',
              'cache': 'OS caches left untouched; fixtures just created',
              'excludes': ['Spotlight subprocess', 'Flutter', 'real GUI'],
              'baseline': measure(binary, ['--baseline']), 'cases': []}
    for entries, matches in [(10_000, 100), (40_000, 100), (10_000, 10_000)]:
        with tempfile.TemporaryDirectory(prefix='anywhere-search-benchmark-') as directory:
            root = Path(directory)
            for n in range(100):
                (root / f'directory-{n:03}').mkdir()
            for n in range(entries):
                name = f'report-{n:06}.txt' if n < matches else f'other-{n:06}.txt'
                (root / f'directory-{n % 100:03}' / name).touch()
            samples = [measure(binary, [str(root), 'report']) for _ in range(3)]
            assert all(sample['matches'] == matches and sample['issues'] == 0 for sample in samples)
            result['cases'].append({'files': entries, 'expected_matches': matches, 'samples': samples})
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
