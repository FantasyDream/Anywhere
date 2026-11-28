import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';

// The repository example build installs this pinned bundler once for both examples.
const require = createRequire(
  new URL('../json-workspace/package.json', import.meta.url),
);
const {build} = require('esbuild');
const sdk = fileURLToPath(
  new URL('../../../packages/plugin-sdk/dist/background.js', import.meta.url),
);
await build({
  entryPoints: ['background.mjs'],
  alias: {'@anywhere/sdk': sdk},
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  outfile: 'dist/background.js',
  minify: true,
});
