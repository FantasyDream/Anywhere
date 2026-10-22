#!/usr/bin/env python3
"""Build ordinary installable examples; does not install or approve them."""
from pathlib import Path
import json
import shutil
import subprocess
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'build/web-examples'
OUT.mkdir(parents=True, exist_ok=True)
def run(args, path):
    subprocess.run(args, cwd=path, check=True)
run(['python3','scripts/generate-design-tokens.py','--check'], ROOT)
sdk = ROOT / 'packages/plugin-sdk'
run(['npm','ci','--registry=https://registry.npmjs.org','--ignore-scripts','--no-audit','--no-fund'],sdk)
run(['npm','run','build'],sdk)
json_tool = ROOT / 'plugins/examples/json-workspace'
run(['npm','ci','--registry=https://registry.npmjs.org','--no-audit','--no-fund'], json_tool)
run(['npm','test'],json_tool)
run(['npm','run','build'],json_tool)
text_tool = ROOT / 'plugins/examples/web-text'
shutil.copytree(sdk / 'dist', text_tool / 'dist/sdk', dirs_exist_ok=True,
                ignore=shutil.ignore_patterns('*.map','*.d.ts'))
shutil.copytree(sdk / 'styles', text_tool / 'dist/sdk/styles', dirs_exist_ok=True)
for directory in [json_tool,text_tool]:
    manifest = json.loads((directory / 'manifest.json').read_text())
    destination = OUT / (manifest['id'] + '.awplugin')
    run(['python3',sdk / 'tools/package.py',directory,'--output',destination],ROOT)
