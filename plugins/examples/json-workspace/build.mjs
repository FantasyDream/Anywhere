import {build} from 'esbuild';
import {mkdir, writeFile, copyFile} from 'node:fs/promises';
await mkdir('dist', {recursive: true});
await build({
  entryPoints: ['src/main.jsx'],
  bundle: true,
  format: 'esm',
  target: 'safari15',
  outfile: 'dist/main.js',
  minify: true,
  jsxFactory: 'h',
  jsxFragment: 'Fragment',
  legalComments: 'eof',
});
await writeFile(
  'dist/index.html',
  '<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="./main.css"></head><body><div id="app"></div><script type="module" src="./main.js"></script></body></html>',
);
await copyFile('node_modules/preact/LICENSE', 'dist/preact-LICENSE.txt');
await copyFile(
  'node_modules/jsonc-parser/LICENSE.md',
  'dist/jsonc-parser-LICENSE.txt',
);

await copyFile(
  'node_modules/jsonpath-js/LICENSE',
  'dist/jsonpath-js-LICENSE.txt',
);

await copyFile(
  'node_modules/@nyariv/sandboxjs/LICENSE',
  'dist/sandboxjs-LICENSE.txt',
);
await copyFile('node_modules/acorn/LICENSE', 'dist/acorn-LICENSE.txt');

await copyFile('node_modules/i18next/LICENSE', 'dist/i18next-LICENSE.txt');
