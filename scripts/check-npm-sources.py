#!/usr/bin/env python3
"""Keep committed npm locks installable outside a contributor's private network."""
import json
from pathlib import Path
import subprocess
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parent.parent


def check_lock(path):
    failures = []
    for name, entry in json.loads(path.read_text()).get('packages', {}).items():
        source = entry.get('resolved')
        if not source or entry.get('link') or source.startswith('file:'):
            continue  # Local SDK snapshots are not registry downloads.
        url = urlsplit(source)
        if (url.scheme != 'https' or url.netloc != 'registry.npmjs.org'
                or url.query or url.fragment):
            # Do not print URLs that might contain credentials or query tokens.
            failures.append(f'{name}: dependency must use the public npm registry')
    return failures


def main():
    names = subprocess.check_output(
        ['git', 'ls-files', '-z', '*package-lock.json'], cwd=ROOT).split(b'\0')
    count = 0
    failures = []
    for name in names:
        if not name:
            continue
        path = ROOT / name.decode()
        if not path.is_file():
            continue
        count += 1
        failures.extend(f'{path.relative_to(ROOT)}: {failure}' for failure in check_lock(path))
    if failures:
        raise SystemExit('\n'.join(failures))
    print(f'npm dependency sources OK: {count} public-registry lockfiles')


if __name__ == '__main__':
    main()
