import {build} from 'esbuild';
await build({
  entryPoints: ['src/main.ts'],
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  conditions: ['anywhere-background'],
  target: 'es2022',
  outfile: 'dist/background.js',
  minify: true,
});
