#!/usr/bin/env python3
"""R3 field-matching comparison, synthetic catalog + registered Action fields.

Includes native preparation as a control to distinguish caching from library gains.
No contexts, authorization, ranking sort, Bridge, GUI or user data are processed.
"""
import argparse
import json
import platform
import statistics
from pathlib import Path
from bench_probe import build_core_probe, measure_probe

TEST = 'core::search::matcher_probe::benchmark::compare'
BACKENDS = ['legacy', 'existing', 'prepared', 'nucleo', 'fuzzy']


def measure(binary, backend, kind='apps', count=1000):
    return measure_probe(binary, TEST, {
        'ANYWHERE_MATCHER_BACKEND': backend, 'ANYWHERE_MATCHER_CORPUS': kind,
        'ANYWHERE_MATCHER_ROWS': str(count)}, 'MATCHER_METRICS ')


def signature(sample):
    return [(q['query'], q['matches'], q['candidate_tiers_sha256']) for q in sample['queries']]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=3)
    options = parser.parse_args()
    if platform.system() != 'Darwin' or options.samples < 1:
        parser.error('requires macOS and at least one sample')
    binary = build_core_probe()
    result = {
        'platform': platform.platform(), 'probe': 'field-level app/action matching',
        'library': 'nucleo-matcher 0.3.1, dev-only, ASCII substring + native Unicode fallback',
        'samples_per_query': 200, 'warmup_per_query': 30,
        'excluded': ['Context resolution', 'availability', 'candidate construction',
                     'ranking sort', 'Bridge', 'Flutter', 'GUI'],
        'baseline': measure(binary, 'baseline'), 'cases': [],
    }
    for kind, count in [('apps',1000), ('apps',10000), ('actions',2000)]:
        samples = {backend: [] for backend in BACKENDS}
        for sample in range(options.samples):
            for backend in (BACKENDS if sample % 2 == 0 else list(reversed(BACKENDS))):
                samples[backend].append(measure(binary, backend, kind, count))
        reference = signature(samples['existing'][0])
        for backend in ['legacy','existing','prepared','nucleo']:
            assert all(signature(s) == reference for s in samples[backend]), backend
        assert all(signature(s) == signature(samples['fuzzy'][0]) for s in samples['fuzzy'])
        result['cases'].append({'corpus':kind,'rows':count,'samples':samples})
        print(json.dumps({'corpus':kind,'rows':count,**{
            backend: {'mean_query_p95_us': statistics.mean(statistics.median(
                s['queries'][q]['p95_us'] for s in samples[backend]) for q in range(len(reference))),
                'rss_bytes': statistics.median(s['max_rss_bytes'] for s in samples[backend])}
            for backend in BACKENDS}}, ensure_ascii=False), flush=True)
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(result, indent=2)+'\n')


if __name__ == '__main__':
    main()
