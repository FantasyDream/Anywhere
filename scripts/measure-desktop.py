#!/usr/bin/env python3
"""Read opt-in UI timings and bounded process samples from a local acceptance run."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import time

OPERATIONS = ('activation', 'query', 'file_query', 'clipboard_query',
              'shelf_query')
MAX_SAMPLES = 10000


def timings(lines, limit=MAX_SAMPLES):
    samples = {operation: [] for operation in OPERATIONS}
    rejected = hidden = overflow = 0
    for line in lines:
        try:
            event = json.loads(line)
        except (ValueError, TypeError):
            continue
        if not isinstance(event,
                          dict) or event.get('event') != 'ui_performance':
            continue
        operation = event.get('operation')
        value = event.get('elapsed_us')
        if operation not in samples or type(value) is not int or value < 0:
            rejected += 1
        elif event.get('native_visible') is not True:
            hidden += 1
        elif len(samples[operation]) >= limit:
            overflow += 1
        else:
            samples[operation].append(value)
    result = {}
    for operation, values in samples.items():
        if not values:
            continue
        values.sort()
        result[operation] = {
            'count': len(values),
            'p50_us': values[(len(values) * 50 + 99) // 100 - 1],
            'p95_us': values[(len(values) * 95 + 99) // 100 - 1],
            'max_us': values[-1],
        }
    return {
        'operations': result,
        'rejected': rejected,
        'hidden': hidden,
        'overflow': overflow
    }


def process_sample(pid):
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,comm='],
                                     text=True)
    processes = {}
    for line in output.splitlines():
        fields = line.split(None, 3)
        if len(fields) == 4 and all(value.isdecimal() for value in fields[:3]):
            number, parent, rss = map(int, fields[:3])
            processes[number] = (parent, rss * 1024, fields[3])
    if pid not in processes:
        return None
    family = {pid}
    while True:
        children = {
            number for number, value in processes.items() if value[0] in family
        }
        following = family | children
        if following == family:
            break
        family = following
    return {
        'main_rss_bytes':
            processes[pid][1],
        'descendants': [{
            'pid': number,
            'name': Path(processes[number][2]).name,
            'rss_bytes': processes[number][1],
        } for number in sorted(family - {pid})],
    }


def footprint(pid):
    result = subprocess.run(['vmmap', '-summary', str(pid)],
                            text=True,
                            capture_output=True)
    match = re.search(r'^Physical footprint:\s*([\d.]+)([KMG]?)', result.stdout,
                      re.MULTILINE)
    if result.returncode or match is None:
        return None
    return round(
        float(match[1]) * {
            '': 1,
            'K': 1024,
            'M': 1024**2,
            'G': 1024**3
        }[match[2]])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--log', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--duration', type=int, default=0)
    parser.add_argument('--interval', type=int, default=5)
    parser.add_argument('--footprint', action='store_true')
    args = parser.parse_args()
    if args.pid < 1 or not 0 <= args.duration <= 3600 or not 1 <= args.interval <= 60:
        parser.error(
            'Use a positive PID, 0–3600 seconds and a 1–60 second interval.')
    start = time.monotonic()
    samples = []
    initial = footprint(args.pid) if args.footprint else None
    while True:
        sample = process_sample(args.pid)
        if sample is None:
            break
        samples.append({
            'elapsed_seconds': round(time.monotonic() - start, 2),
            **sample
        })
        remaining = args.duration - (time.monotonic() - start)
        if remaining <= 0:
            break
        time.sleep(min(args.interval, remaining))
    with args.log.open(encoding='utf-8', errors='replace') as stream:
        latency = timings(stream)
    report = {
        'scope':
            'Committed input through Rust/bridge and Flutter post-frame; excludes physical display presentation.',
        'latency':
            latency,
        'resources':
            samples,
        'initial_footprint_bytes':
            initial,
        'final_footprint_bytes':
            footprint(args.pid) if args.footprint else None,
        'limits':
            'RSS and footprint are separate metrics. Descendants are attributed by PID ancestry only; unparented XPC helpers are not counted. Not a leak or long-term stability proof.',
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Recorded {len(samples)} process samples; UI sample counts: ' +
          ', '.join(f'{name}={value["count"]}'
                    for name, value in latency['operations'].items()))


if __name__ == '__main__':
    main()
