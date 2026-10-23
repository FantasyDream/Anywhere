import {build} from 'esbuild';
import {mkdir, copyFile} from 'node:fs/promises';
await mkdir('dist', {recursive: true});
await build({
  entryPoints: ['src/main.js'],
  bundle: true,
  format: 'esm',
  target: 'safari15',
  outfile: 'dist/main.js',
  minify: true,
});
await copyFile('index.html', 'dist/index.html');
