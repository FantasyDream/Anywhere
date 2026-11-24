#!/usr/bin/env python3
"""Check the actual Cargo dependency graph, including renamed/transitive dependencies."""
import json
from pathlib import Path
import subprocess
from architecture_rules import check_core_imports, check_feature_imports, check_lock_usage, check_persistence_imports, production_source

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
        for term in ['gpui', 'webview', 'swiftui', 'winui', 'gtk']), name
    for dependency in nodes[package_id]['dependencies']:
        walk(dependency)


walk(core)
print(f'Architecture OK: {len(visited)} core dependencies, no UI dependency')

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
    if source.name == 'tests.rs' or source.name.endswith('_tests.rs'):
        continue
    check_lock_usage(source.read_text(), source)
    production = production_source(source.read_text())
    for token in ['MacOsPlatform', 'com.apple.finder', '/bin/sh', 'std::os::']:
        assert token not in production, (source, token)

print(
    'Platform boundaries OK: shared Core has no UI or native packaging dependency'
)
