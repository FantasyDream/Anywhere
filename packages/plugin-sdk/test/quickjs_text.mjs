import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const require = createRequire(
  new URL(
    '../../../plugins/examples/background-context/package.json',
    import.meta.url,
  ),
);
const {build} = require('esbuild');
const fixture = `${root}build/plugin-sdk-text`;
await mkdir(`${fixture}/dist`, {recursive: true});
const manifest = JSON.parse(
  await readFile(
    `${root}plugins/examples/background-context/manifest.json`,
    'utf8',
  ),
);
manifest.permissions.push({name: 'clipboard.readText'});
await writeFile(`${fixture}/manifest.json`, JSON.stringify(manifest));
const archive = `${fixture}/text-upload.awplugin`;
for (const name of ['small', 'unicode', 'limit', 'escaped', 'invalid']) {
  // Each sample has its own finite job; do not combine their JS/memory budgets.
  await build({
    entryPoints: [
      fileURLToPath(new URL('fixtures/text-upload.js', import.meta.url)),
    ],
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    target: 'es2022',
    minify: true,
    define: {'globalThis.__textUploadCase': JSON.stringify(name)},
    outfile: `${fixture}/dist/background.js`,
  });
  execFileSync(
    'python3',
    [
      `${root}packages/plugin-sdk/tools/package.py`,
      fixture,
      '--output',
      archive,
    ],
    {
      cwd: root,
      stdio: 'inherit',
    },
  );
  execFileSync(
    'cargo',
    [
      'run',
      // Keep the production 250 ms budget; debug broker parsing adds overhead
      // outside the paused host I/O and cannot establish full-MiB conformance.
      '--release',
      '-p',
      'anywhere-core',
      '--example',
      'web_sdk_conformance',
      '--locked',
      '--',
      '--background',
      archive,
    ],
    {
      cwd: root,
      stdio: 'inherit',
    },
  );
  console.log(`QuickJS text conformance: ${name} passed`);
}
console.log('QuickJS text conformance passed (release, including full MiB)');
