#!/usr/bin/env python3
"""Check the actual Cargo dependency graph, including renamed/transitive dependencies."""
import json
from pathlib import Path
import re
import subprocess

metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1']))
packages = {package['id']: package for package in metadata['packages']}
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
core = next(package['id'] for package in packages.values() if package['name'] == 'anywhere-core')
visited = set()

def walk(package_id):
    if package_id in visited:
        return
    visited.add(package_id)
    name = packages[package_id]['name']
    assert not any(term in name.lower() for term in ['flutter', 'dart', 'webview', 'swiftui', 'winui', 'gtk']), name
    for dependency in nodes[package_id]['dependencies']:
        walk(dependency)

walk(core)
print(f'Architecture OK: {len(visited)} core dependencies, no UI/Flutter bridge dependency')

# Production modules must not import product composition or feature implementations.
# The existing Rust convention keeps each inline test module at the end of its file.
for source in Path('crates/anywhere-core/src/core').rglob('*.rs'):
    production = source.read_text().split('#[cfg(test)]', 1)[0]
    production = re.sub(r'//[^\n]*|/\*.*?\*/', '', production, flags=re.S)
    forbidden = r'\b(features|application|persistence)\b'
    assert not re.search(forbidden + r'\s*::', production), source
    for imported in re.findall(r'\buse\s+([^;]+);', production):
        assert not re.search(forbidden, imported), source
print('Module imports OK: core has no product/feature/persistence dependency')

# Shared product logic cannot select OS adapters or assume a macOS application identity.
for source in Path('crates/anywhere-core/src').rglob('*.rs'):
    production = source.read_text().split('#[cfg(test)]', 1)[0]
    production = re.sub(r'//[^\n]*|/\*.*?\*/', '', production, flags=re.S)
    for token in ['MacOsPlatform', 'com.apple.finder', '/bin/sh', 'std::os::']:
        assert token not in production, (source, token)

# UI widgets and startup do not guess native packaging, fonts, or key encodings.
# Platform presentation policy and read-only legacy migration belong to infrastructure.
for source in Path('apps/flutter/lib').rglob('*.dart'):
    if 'src/rust' in str(source) or 'infrastructure' in source.parts:
        continue
    for token in ['.AppleSystemUIFont', 'carbonEventKeyCode', 'macOSPhysicalKeyCodes',
                  'Platform.isMacOS', 'Platform.resolvedExecutable', 'Frameworks/']:
        assert token not in source.read_text(), (source, token)
print('Platform boundaries OK: shared Core and UI do not select native OS implementations')
