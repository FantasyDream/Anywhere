#!/usr/bin/env python3
"""Keep compiled feature metadata and the native/Flutter App release aligned."""
import argparse
import json
from pathlib import Path
import plistlib
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / 'apps/flutter'


def bridge_runtime_version(root):
    locations = [
        (root / 'apps/flutter/pubspec.yaml',
         r'^\s+flutter_rust_bridge:\s*(\S+)\s*$'),
        (root / 'crates/anywhere-bridge-flutter/Cargo.toml',
         r'^flutter_rust_bridge\s*=\s*"=([^\"]+)"\s*$'),
    ]
    return matching_bridge_versions(locations)


def matching_bridge_versions(locations, expected=None):
    for path, pattern in locations:
        match = re.search(pattern, path.read_text(), re.MULTILINE)
        if match is None:
            raise SystemExit(f'FRB version metadata is missing: {path.name}')
        version = match.group(1)
        if expected is not None and version != expected:
            raise SystemExit(
                f'FRB versions differ: {path.name} has {version}, expected {expected}. '
                'Align both runtimes and regenerate bindings with the matching generator.'
            )
        expected = version
    return expected


def check_bridge_versions(root):
    version = bridge_runtime_version(root)
    matching_bridge_versions([
        (root / 'apps/flutter/lib/src/rust/frb_generated.dart',
         r"String get codegenVersion => '([^']+)';"),
        (root / 'crates/anywhere-bridge-flutter/src/frb_generated.rs',
         r'FLUTTER_RUST_BRIDGE_CODEGEN_VERSION: &str = "([^\"]+)";'),
    ], version)
    return version


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bridge-runtime-version', action='store_true')
    if parser.parse_args().bridge_runtime_version:
        print(bridge_runtime_version(ROOT))
        return
    info = plistlib.loads((APP / 'macos/Runner/Info.plist').read_bytes())
    release = info['CFBundleShortVersionString']
    build = info['CFBundleVersion']
    expected = f'{release}+{build}'
    flutter = re.search(r'^version:\s*(\S+)\s*$',
                        (APP / 'pubspec.yaml').read_text(), re.MULTILINE)
    if flutter is None or flutter.group(1) != expected:
        raise SystemExit(
            f'Flutter and native App versions differ; expected {expected}')
    metadata = json.loads(
        subprocess.check_output([
            'cargo', 'metadata', '--locked', '--no-deps', '--format-version',
            '1'
        ],
                                cwd=ROOT,
                                text=True))
    members = set(metadata['workspace_members'])
    for package in metadata['packages']:
        if package['id'] in members and package['version'] != release:
            raise SystemExit(
                f"{package['name']} version {package['version']} differs "
                f'from App release {release}')
    print(f'App versions OK: Rust {release}, Flutter/native {expected}')
    print(f'FRB versions OK: Dart/Rust/generated {check_bridge_versions(ROOT)}')


if __name__ == '__main__':
    main()
