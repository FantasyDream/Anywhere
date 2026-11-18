#!/usr/bin/env python3
"""Keep the Cargo workspace and macOS app release metadata aligned."""
import json
from pathlib import Path
import plistlib
import subprocess

ROOT = Path(__file__).resolve().parent.parent
INFO = ROOT / 'apps/gpui/macos/Info.plist'


def check_workspace_versions(metadata, release: str) -> None:
    members = set(metadata['workspace_members'])
    for package in metadata['packages']:
        if package['id'] in members and package['version'] != release:
            raise SystemExit(
                f"{package['name']} version {package['version']} differs "
                f'from App release {release}')


def main() -> None:
    info = plistlib.loads(INFO.read_bytes())
    release = info['CFBundleShortVersionString']
    build = info['CFBundleVersion']
    if not isinstance(build, str) or not build.isdecimal():
        raise SystemExit(
            'CFBundleVersion must be a positive decimal build number.')
    metadata = json.loads(
        subprocess.check_output([
            'cargo', 'metadata', '--locked', '--no-deps', '--format-version',
            '1'
        ],
                                cwd=ROOT,
                                text=True))
    check_workspace_versions(metadata, release)
    print(f'App versions OK: workspace {release}, macOS build {build}')


if __name__ == '__main__':
    main()
