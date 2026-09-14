#!/usr/bin/env python3
"""Check the actual Cargo dependency graph, including renamed/transitive dependencies."""
import json
import subprocess

metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1']))
packages = {package['id']: package for package in metadata['packages']}
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
core = next(package['id'] for package in packages.values() if package['name'] == 'launcher-core')
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
