#!/usr/bin/env node
import {cp, mkdir, readFile, writeFile, lstat, rm} from 'node:fs/promises';
import {dirname, resolve, relative, basename, isAbsolute} from 'node:path';
import {fileURLToPath} from 'node:url';

const sdk = resolve(dirname(fileURLToPath(import.meta.url)), '..');
async function init(destination) {
  const target = resolve(destination),
    inside = relative(sdk, target);
  if (!inside || (!inside.startsWith('..') && !isAbsolute(inside)))
    throw new Error('Choose a directory outside the SDK.');
  const slug = basename(target);
  if (!/^[a-z][a-z0-9-]{0,47}$/.test(slug))
    throw new Error(
      'Use a directory name such as my-tool (lowercase letters, numbers and hyphens).',
    );
  try {
    await lstat(target);
    throw new Error('Destination already exists. Choose a new directory.');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  await mkdir(target, {recursive: true});
  try {
    await cp(resolve(sdk, 'templates/minimal'), target, {recursive: true});
    const vendor = resolve(target, 'vendor/plugin-sdk');
    await mkdir(vendor, {recursive: true});
    for (const entry of [
      'dist',
      'styles',
      'tools',
      'templates',
      'package.json',
      'README.md',
      'LICENSE',
    ]) {
      await cp(resolve(sdk, entry), resolve(vendor, entry), {recursive: true});
    }
    const pkg = JSON.parse(
      await readFile(resolve(target, 'package.json'), 'utf8'),
    );
    pkg.name = `anywhere-${slug}`;
    pkg.devDependencies['@anywhere/plugin-sdk'] = 'file:vendor/plugin-sdk';
    pkg.scripts.pack = `python3 node_modules/@anywhere/plugin-sdk/tools/package.py . --output output/${slug}.awplugin`;
    await writeFile(
      resolve(target, 'package.json'),
      JSON.stringify(pkg, null, 2) + '\n',
    );
    await writeFile(
      resolve(target, '.gitignore'),
      'node_modules/\ndist/\noutput/\n.DS_Store\n',
    );
    const manifest = JSON.parse(
      await readFile(resolve(target, 'manifest.json'), 'utf8'),
    );
    manifest.id = `dev.example.${slug}`;
    manifest.name = manifest.commands[0].title = slug
      .split('-')
      .map(word => word[0].toUpperCase() + word.slice(1))
      .join(' ');
    await writeFile(
      resolve(target, 'manifest.json'),
      JSON.stringify(manifest, null, 2) + '\n',
    );
  } catch (error) {
    await rm(target, {recursive: true, force: true});
    throw error;
  }
  process.stdout.write(
    `Created ${target}\n\ncd ${destination}\nnpm install --ignore-scripts\nnpm run build\nnpm run pack\n\nEdit src/main.js for business logic. Host styling and lifecycle are automatic.\n`,
  );
}

try {
  const [command, destination, ...extra] = process.argv.slice(2);
  if (command !== 'init' || !destination || extra.length)
    throw new Error('Usage: anywhere-plugin init my-tool');
  await init(destination);
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
