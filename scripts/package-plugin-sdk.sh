#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
OUT="$ROOT/build/plugin-sdk"
mkdir -p "$OUT"
(
  cd packages/plugin-sdk
  npm pack --json --pack-destination "$OUT"
) >"$OUT/pack.json"
node --input-type=module - "$OUT" <<'JS'
import {createHash} from 'node:crypto';
import {readFile, writeFile} from 'node:fs/promises';
import {join} from 'node:path';

const out = process.argv[2];
const [result] = JSON.parse(await readFile(join(out, 'pack.json'), 'utf8'));
const files = new Set(result.files.map(file => file.path));
for (const path of [
  'dist/index.js',
  'dist/background.js',
  'dist/mock.js',
  'tools/anywhere-plugin.mjs',
  'tools/unicode-case-fold.json',
  'tools/skill/SKILL.md',
  'schemas/web-plugin-manifest-editor.schema.json',
  'schemas/web-plugin-bridge.schema.json',
  'LICENSE',
]) {
  if (!files.has(path)) throw new Error(`SDK archive is missing ${path}`);
}
if ([...files].some(path => path.endsWith('package.py') || path.includes('vendor/'))) {
  throw new Error('SDK archive contains retired or redundant files');
}
const archive = await readFile(join(out, result.filename));
const sha256 = createHash('sha256').update(archive).digest('hex');
await writeFile(join(out, `${result.filename}.sha256`), `${sha256}  ${result.filename}\n`);
console.log(join(out, result.filename));
JS
