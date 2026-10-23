#!/usr/bin/env python3
"""Check the actual Cargo dependency graph, including renamed/transitive dependencies."""
import json
from pathlib import Path
import re
import subprocess
from architecture_rules import check_core_imports, check_feature_imports, check_persistence_imports

subprocess.check_call([
    'python3', '-m', 'unittest', 'discover', '-s', 'scripts/tests', '-p',
    'test_architecture_rules.py'
])

metadata = json.loads(
    subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--format-version=1']))
packages = {package['id']: package for package in metadata['packages']}
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
core = next(package['id']
            for package in packages.values()
            if package['name'] == 'anywhere-core')
visited = set()


def walk(package_id):
    if package_id in visited:
        return
    visited.add(package_id)
    name = packages[package_id]['name']
    assert not any(
        term in name.lower()
        for term in ['flutter', 'dart', 'webview', 'swiftui', 'winui', 'gtk'
                    ]), name
    for dependency in nodes[package_id]['dependencies']:
        walk(dependency)


walk(core)
print(
    f'Architecture OK: {len(visited)} core dependencies, no UI/Flutter bridge dependency'
)

# Production modules must not import product composition or feature implementations.
# The existing Rust convention keeps each inline test module at the end of its file.
for source in Path('crates/anywhere-core/src/core').rglob('*.rs'):
    check_core_imports(source.read_text(), source)
print('Module imports OK: core has no product/feature/persistence dependency')

# Feature services depend on repository contracts, never concrete app/storage wiring.
for source in Path('crates/anywhere-core/src/features').rglob('*.rs'):
    if source.name.endswith('_tests.rs'):
        continue  # These files are included only by cfg(test) modules.
    check_feature_imports(source.read_text(), source)
print('Feature imports OK: no application or concrete persistence dependency')

for source in Path('crates/anywhere-core/src/persistence').rglob('*.rs'):
    check_persistence_imports(source.read_text(), source)
print('Persistence imports OK: adapters do not call Application')

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
    for token in [
            '.AppleSystemUIFont', 'carbonEventKeyCode', 'macOSPhysicalKeyCodes',
            'Platform.isMacOS', 'Platform.resolvedExecutable', 'Frameworks/'
    ]:
        assert token not in source.read_text(), (source, token)
print(
    'Platform boundaries OK: shared Core and UI do not select native OS implementations'
)
