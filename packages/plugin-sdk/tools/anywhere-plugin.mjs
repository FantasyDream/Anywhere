#!/usr/bin/env node
import {cp, mkdir, readFile, writeFile, rename, rm} from 'node:fs/promises';
import {parseArgs} from 'node:util';
import {check, readManifest} from './manifest.mjs';
import {pack, validateBuilt} from './pack.mjs';
import {dirname, resolve, relative, basename, isAbsolute} from 'node:path';
import {fileURLToPath} from 'node:url';

check(
  Number(process.versions.node.split('.')[0]) >= 22,
  'Anywhere CLI requires Node.js 22 or newer',
);
const sdk = resolve(dirname(fileURLToPath(import.meta.url)), '..');
async function init(destination, template) {
  const background = template === 'background';
  const hybrid = template === 'hybrid';
  const typescript = template === 'ts';
  const target = resolve(destination),
    inside = relative(sdk, target);
  if (!inside || (!inside.startsWith('..') && !isAbsolute(inside)))
    throw new Error('Choose a directory outside the SDK.');
  const slug = basename(target);
  if (!/^[a-z][a-z0-9-]{0,47}$/.test(slug))
    throw new Error(
      'Use a directory name such as my-tool (lowercase letters, numbers and hyphens).',
    );
  await mkdir(dirname(target), {recursive: true});
  await mkdir(target).catch(error => {
    if (error.code === 'EEXIST')
      throw new Error('Destination already exists. Choose a new directory.');
    throw error;
  });
  try {
    await cp(
      resolve(sdk, background ? 'templates/background' : 'templates/minimal'),
      target,
      {recursive: true},
    );
    await readFile(resolve(sdk, 'dist/index.js')).catch(() => {
      throw new Error(
        'Plugin SDK is not built yet. Run `npm run build` in packages/plugin-sdk first.',
      );
    });
    const vendor = resolve(target, 'vendor/plugin-sdk');
    await mkdir(vendor, {recursive: true});
    for (const entry of [
      'dist',
      'styles',
      'tools',
      'package.json',
      'README.md',
      'LICENSE',
    ]) {
      await cp(resolve(sdk, entry), resolve(vendor, entry), {recursive: true});
    }
    const schemas = resolve(vendor, 'schemas');
    await mkdir(schemas);
    for (const name of [
      'web-plugin-manifest.schema.json',
      'web-plugin-manifest-editor.schema.json',
      'web-plugin-bridge.schema.json',
    ]) {
      await cp(resolve(sdk, 'schemas', name), resolve(schemas, name)).catch(
        error => {
          if (error.code !== 'ENOENT') throw error;
          return cp(
            resolve(sdk, '../../resources', name),
            resolve(schemas, name),
          );
        },
      );
    }
    const skill = resolve(target, '.agents/skills/anywhere-plugin-dev');
    await mkdir(skill, {recursive: true});
    await cp(resolve(sdk, 'tools/skill/SKILL.md'), resolve(skill, 'SKILL.md'));
    const pkg = JSON.parse(
      await readFile(resolve(target, 'package.json'), 'utf8'),
    );
    pkg.name = `anywhere-${slug}`;
    pkg.devDependencies['@anywhere/sdk'] = 'file:vendor/plugin-sdk';
    pkg.scripts.pack = `anywhere-plugin pack --output output/${slug}.awplugin`;
    pkg.scripts.validate = 'anywhere-plugin validate';
    pkg.scripts.test = 'anywhere-plugin test';
    pkg.scripts.dev = background
      ? 'npm run build && anywhere-plugin test'
      : 'npm run build && node node_modules/@anywhere/sdk/tools/dev.mjs';
    if (hybrid || typescript) {
      pkg.devDependencies.typescript = '7.0.2';
      pkg.scripts.check = 'tsc --noEmit';
      await cp(
        resolve(sdk, 'templates/background/tsconfig.json'),
        resolve(target, 'tsconfig.json'),
      );
    }
    if (typescript) {
      const src = resolve(target, 'src/main.js');
      const source = (await readFile(src, 'utf8'))
        .replace(
          "querySelector('#input')",
          "querySelector<HTMLTextAreaElement>('#input')!",
        )
        .replace(
          "querySelector('#status')",
          "querySelector<HTMLOutputElement>('#status')!",
        )
        .replace(
          "querySelector('#copy')",
          "querySelector<HTMLButtonElement>('#copy')!",
        )
        .replace(
          "querySelector('#uppercase')",
          "querySelector<HTMLButtonElement>('#uppercase')!",
        );
      await rename(src, resolve(target, 'src/main.ts'));
      await writeFile(resolve(target, 'src/main.ts'), source);
      const build = resolve(target, 'build.mjs');
      await writeFile(
        build,
        (await readFile(build, 'utf8')).replace('src/main.js', 'src/main.ts'),
      );
      const config = JSON.parse(
        await readFile(resolve(target, 'tsconfig.json'), 'utf8'),
      );
      config.compilerOptions.lib.push('DOM');
      delete config.compilerOptions.customConditions;
      await writeFile(
        resolve(target, 'tsconfig.json'),
        JSON.stringify(config, null, 2) + '\n',
      );
    }
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
    if (background || hybrid) {
      const templateRoot = resolve(sdk, 'templates/background');
      const source = await readFile(
        resolve(templateRoot, 'src/main.ts'),
        'utf8',
      );
      const backgroundManifest = JSON.parse(
        await readFile(resolve(templateRoot, 'manifest.json'), 'utf8'),
      );
      const backgroundSource = background ? 'src/main.ts' : 'src/background.ts';
      await writeFile(
        resolve(target, backgroundSource),
        source.replaceAll(backgroundManifest.id, manifest.id),
      );
      const updated = JSON.parse(
        JSON.stringify(backgroundManifest).replaceAll(
          `plugin.${backgroundManifest.id}.`,
          `plugin.${manifest.id}.`,
        ),
      );
      manifest.background = updated.background;
      if (hybrid) {
        for (const permission of updated.permissions) {
          const existing = manifest.permissions.find(
            entry => entry.name === permission.name,
          );
          if (!existing) manifest.permissions.push(permission);
          else if (permission.types)
            existing.types = [
              ...new Set([...existing.types, ...permission.types]),
            ];
        }
        manifest.commands.push(...updated.commands);
        const backgroundBuild = await readFile(
          resolve(templateRoot, 'build.mjs'),
          'utf8',
        );
        await writeFile(
          resolve(target, 'build-background.mjs'),
          backgroundBuild.replace('src/main.ts', backgroundSource),
        );
        pkg.scripts.build += ' && node build-background.mjs';
        await writeFile(
          resolve(target, 'package.json'),
          JSON.stringify(pkg, null, 2) + '\n',
        );
      } else {
        manifest.permissions = updated.permissions;
        manifest.commands = updated.commands;
      }
    }
    manifest.$schema =
      './vendor/plugin-sdk/schemas/web-plugin-manifest-editor.schema.json';
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
    `Created ${target}\n\ncd ${destination}\nnpm install --ignore-scripts\nnpm run build\nnpm run pack\n\nEdit src/main.${background || typescript ? 'ts' : 'js'} for business logic. Host styling and lifecycle are automatic.\n`,
  );
}

const usage =
  'Usage: anywhere-plugin init <dir> [--ts | --background | --hybrid] | validate [dir] [--built] | pack [dir] --output <file.awplugin> | test [dir]';

try {
  const [command, ...args] = process.argv.slice(2);
  const commands = {
    init: {
      background: {type: 'boolean'},
      ts: {type: 'boolean'},
      hybrid: {type: 'boolean'},
    },
    validate: {built: {type: 'boolean'}},
    pack: {output: {type: 'string'}},
    test: {},
  };
  if (command === '--help' || !command) {
    process.stdout.write(usage + '\n');
  } else {
    check(Object.hasOwn(commands, command), usage);
    const {positionals, values} = parseArgs({
      args,
      allowPositionals: true,
      options: commands[command],
    });
    check(positionals.length <= 1, usage);
    const [directory = '.'] = positionals;
    if (command === 'init') {
      check(
        positionals.length === 1 &&
          Object.values(values).filter(Boolean).length <= 1,
        usage,
      );
      await init(directory, Object.keys(values)[0] ?? 'web');
    } else {
      const project = resolve(directory);
      const manifest = await readManifest(project, message =>
        process.stderr.write(`Warning: ${message}\n`),
      );
      if (command === 'pack') {
        check(values.output, usage);
        const size = await pack(project, manifest, values.output);
        process.stdout.write(
          `Built ${resolve(values.output)} (${size} bytes)\n`,
        );
      } else if (command === 'test') {
        const {testProject} = await import('./test-project.mjs');
        await testProject(project, manifest);
      } else {
        if (values.built) await validateBuilt(project, manifest);
        process.stdout.write(`Manifest valid: ${manifest.id}\n`);
      }
    }
  }
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
