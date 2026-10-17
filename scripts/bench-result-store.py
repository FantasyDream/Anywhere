#!/usr/bin/env python3
"""R2 isolated release comparison. No real files, GUI, index or clipboard access.

Builds a test-only backend; neither the prototype nor its helpers ship in the app.
macOS RSS uses /usr/bin/time -l. Each sample gets a separate process/workspace.
"""
import argparse
import json
import os
import platform
import re
import statistics
import subprocess
from pathlib import Path

TEST = 'features::filesystem::result_store_probe::benchmark::compare'


def build():
    process = subprocess.run(
        ['cargo', 'test', '-p', 'anywhere-core', '--release', '--locked',
         '--lib', '--no-run', '--message-format=json'],
        check=True, text=True, stdout=subprocess.PIPE)
    for line in process.stdout.splitlines():
        message = json.loads(line)
        if (message.get('reason') == 'compiler-artifact'
                and message['target']['name'] == 'anywhere_core'
                and message.get('executable')):
            return message['executable']
    raise RuntimeError('release test executable missing')


def measure(binary, backend, count=0, waves=1, cancel=False):
    environment = dict(os.environ, ANYWHERE_PROBE_BACKEND=backend,
                       ANYWHERE_PROBE_ROWS=str(count), ANYWHERE_PROBE_WAVES=str(waves),
                       ANYWHERE_PROBE_CANCEL='1' if cancel else '0')
    process = subprocess.run(
        ['/usr/bin/time', '-l', binary, '--exact', TEST, '--ignored',
         '--nocapture', '--test-threads=1'], env=environment,
        check=True, text=True, capture_output=True)
    metrics = next(line.partition('RESULT_STORE_METRICS ')[2]
                   for line in process.stdout.splitlines() if 'RESULT_STORE_METRICS ' in line)
    result = json.loads(metrics)
    rss = re.search(r'(\d+)\s+maximum resident set size', process.stderr)
    assert rss, process.stderr
    result['max_rss_bytes'] = int(rss[1])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=3)
    options = parser.parse_args()
    if platform.system() != 'Darwin' or options.samples < 1:
        parser.error('requires macOS and at least one sample')
    binary = build()
    result = {
        'platform': platform.platform(), 'probe': 'query-local unread result storage',
        'cache': 'warm OS caches, synthetic raw records; no cache clearing',
        'excluded': ['directory traversal', 'Spotlight', 'Flutter', 'Bridge', 'GUI'],
        'notes': ['both use actual production raw codec and ranking',
                  'first batch includes SQLite open/schema, excludes fixture writes',
                  'temporary disk sampled between batches; not a true peak',
                  'RSS includes the common test harness and displayed path set',
                  'Unix path ordering only; no Windows validation'],
        'baseline': measure(binary, 'baseline'), 'cases': [],
    }
    for count, waves in [(100, 1), (10_000, 1), (100_000, 1), (10_000, 16), (100_000, 16)]:
        samples = {'external': [], 'sqlite': []}
        for sample in range(options.samples):
            # Alternate order to reduce consistent warm-cache/thermal bias.
            for backend in (['external', 'sqlite'] if sample % 2 == 0 else ['sqlite', 'external']):
                samples[backend].append(measure(binary, backend, count, waves))
        all_samples = samples['external'] + samples['sqlite']
        assert all(s['returned'] == count for s in all_samples)
        assert len({s['displayed_order_sha256'] for s in all_samples}) == 1, 'order differs'
        case = {'rows': count, 'waves': waves, 'samples': samples}
        result['cases'].append(case)
        print(json.dumps({'rows': count, 'waves': waves, **{
            key: {metric: statistics.median(s[metric] for s in rows)
                  for metric in ['first_batch_ms', 'all_batches_ms', 'max_rss_bytes', 'temporary_bytes_sampled']}
            for key, rows in samples.items()}}, ensure_ascii=False), flush=True)
    result['cooperative_cancellation'] = {
        backend: [measure(binary, backend, 100_000, cancel=True)
                  for _ in range(options.samples)]
        for backend in ['external', 'sqlite']}
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
