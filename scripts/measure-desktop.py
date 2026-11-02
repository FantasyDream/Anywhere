#!/usr/bin/env python3
"""Read opt-in UI timings and bounded process samples from a local acceptance run."""
import argparse
import json
from pathlib import Path
import platform
import re
import subprocess
import time

OPERATIONS = ('activation', 'query', 'file_query', 'clipboard_query',
              'shelf_query')
MAX_SAMPLES = 10000
FOOTPRINT_TIMEOUT_SECONDS = 15
WEBKIT_INSPECTION_TIMEOUT_SECONDS = 2
MAX_WEBKIT_HELPERS = 8


def webkit_helpers(pid):
    """Verify running WebKit instances in this process's own XPC domain.

    None means inspection is unavailable, not that there are zero helpers.
    Never retain launchctl's environment, endpoints or unrelated services.
    """
    if platform.system() != 'Darwin':
        return None

    def inspect(target):
        result = subprocess.run(['/bin/launchctl', 'print', target],
                                text=True,
                                capture_output=True,
                                timeout=WEBKIT_INSPECTION_TIMEOUT_SECONDS)
        return result.stdout if result.returncode == 0 else None

    try:
        domain = inspect(f'pid/{pid}')
        if domain is None or not domain.startswith(f'pid/{pid} = {{\n'):
            return None
        # launchctl can create an empty domain for an absent process. It must
        # already belong to the requested PID, rather than this inspector.
        header = domain.split('\n\tenvironment = {', 1)[0]
        if re.search(r'^\tcreator = [^\n]*\[' + str(pid) + r'\]$', header,
                     re.MULTILINE) is None:
            return None
        identity = re.search(r'^\tuniqueid = ([1-9]\d*)$', header, re.MULTILINE)
        services = re.search(r'^\tservices = \{\n(.*?)^\t\}', domain,
                             re.MULTILINE | re.DOTALL)
        if identity is None or services is None:
            return None
        entry_pattern = (
            r'([1-9]\d*)\s+\S+\s+'
            r'(com\.apple\.WebKit\.(GPU|WebContent|Networking)\.'
            r'[A-Fa-f0-9]{8}(?:-[A-Fa-f0-9]{4}){3}-[A-Fa-f0-9]{12})')
        entries = []
        for line in services[1].splitlines():
            if re.search(
                    r'com\.apple\.WebKit\.(GPU|WebContent|Networking)(\.|\s|$)',
                    line) is None:
                continue
            if re.fullmatch(r'\s*0\s+\S+\s+\S+\s*', line):
                continue
            entry = re.fullmatch(entry_pattern, line.strip())
            # A changed system format must not look like zero active helpers.
            if entry is None:
                return None
            entries.append(entry.groups())
        if len(entries) > MAX_WEBKIT_HELPERS:
            return None
        helpers = []
        for listed_pid, service, kind in entries:
            job = inspect(f'pid/{pid}/{service}')
            if job is None:
                return None
            actual_pid = re.search(r'^\tpid = (\d+)$', job, re.MULTILINE)
            if actual_pid is None or actual_pid[1] != listed_pid:
                return None
            if re.search(r'^\tstate = running$', job, re.MULTILINE) is None:
                return None
            helpers.append({
                'pid': int(listed_pid),
                'name': f'com.apple.WebKit.{kind}',
            })
        return helpers
    except (OSError, subprocess.TimeoutExpired):
        return None


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


def process_sample(pid, *, include_webkit=False):
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
    sample = {
        'main_rss_bytes':
            processes[pid][1],
        'descendants': [{
            'pid': number,
            'name': Path(processes[number][2]).name,
            'rss_bytes': processes[number][1],
        } for number in sorted(family - {pid})],
    }
    if include_webkit:
        helpers = webkit_helpers(pid)
        if helpers is not None:
            for helper in helpers:
                process = processes.get(helper['pid'])
                if process and Path(process[2]).name != helper['name']:
                    helpers = None
                    break
                # The process list precedes XPC inspection; a just-started
                # helper can be confirmed without having an RSS sample yet.
                helper['rss_bytes'] = process[1] if process else None
        sample['webkit_helpers'] = helpers
    return sample


def footprint(pid):
    try:
        result = subprocess.run(
            ['vmmap', '-summary', str(pid)],
            text=True,
            capture_output=True,
            timeout=FOOTPRINT_TIMEOUT_SECONDS)
    except (OSError, subprocess.TimeoutExpired):
        # A diagnostic must remain optional when task inspection is unavailable
        # or stalls. Missing footprint data does not imply zero memory usage.
        return None
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
    parser.add_argument(
        '--webkit-helpers',
        action='store_true',
        help='Verify macOS WebKit helpers in the target XPC domain')
    args = parser.parse_args()
    if args.pid < 1 or not 0 <= args.duration <= 3600 or not 1 <= args.interval <= 60:
        parser.error(
            'Use a positive PID, 0–3600 seconds and a 1–60 second interval.')
    start = time.monotonic()
    samples = []
    initial = footprint(args.pid) if args.footprint else None
    while True:
        sample = process_sample(args.pid, include_webkit=args.webkit_helpers)
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
            'RSS and footprint are separate metrics. Descendants use PID ancestry. Optional WebKit helpers are verified in the target XPC domain and reported separately; null means inspection was unavailable. Other XPC services are not counted. Not a leak or long-term stability proof.',
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Recorded {len(samples)} process samples; UI sample counts: ' +
          ', '.join(f'{name}={value["count"]}'
                    for name, value in latency['operations'].items()))


if __name__ == '__main__':
    main()
