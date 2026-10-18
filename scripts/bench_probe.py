"""Shared release-test build and isolated macOS RSS measurement for R2 / R3."""
import json
import os
import re
import subprocess


def build_core_probe():
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


def measure_probe(binary, test, variables, marker):
    process = subprocess.run(
        ['/usr/bin/time', '-l', binary, '--exact', test, '--ignored',
         '--nocapture', '--test-threads=1'], env=dict(os.environ, **variables),
        check=True, text=True, capture_output=True)
    metrics = next(line.partition(marker)[2]
                   for line in process.stdout.splitlines() if marker in line)
    result = json.loads(metrics)
    rss = re.search(r'(\d+)\s+maximum resident set size', process.stderr)
    assert rss, process.stderr
    result['max_rss_bytes'] = int(rss[1])
    return result
