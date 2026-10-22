#!/usr/bin/env python3
"""Keep compiled feature metadata and the native/Flutter App release aligned."""
import json
from pathlib import Path
import plistlib
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / 'apps/flutter'


def main():
    info = plistlib.loads((APP / 'macos/Runner/Info.plist').read_bytes())
    release = info['CFBundleShortVersionString']
    build = info['CFBundleVersion']
    expected = f'{release}+{build}'
    flutter = re.search(r'^version:\s*(\S+)\s*$',
                        (APP / 'pubspec.yaml').read_text(), re.MULTILINE)
    if flutter is None or flutter.group(1) != expected:
        raise SystemExit(f'Flutter and native App versions differ; expected {expected}')
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'],
        cwd=ROOT, text=True))
    members = set(metadata['workspace_members'])
    for package in metadata['packages']:
        if package['id'] in members and package['version'] != release:
            raise SystemExit(f"{package['name']} version {package['version']} differs "
                             f'from App release {release}')
    print(f'App versions OK: Rust {release}, Flutter/native {expected}')


if __name__ == '__main__':
    main()
