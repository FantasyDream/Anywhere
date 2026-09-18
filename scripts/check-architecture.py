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
