import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {check} from './manifest.mjs';
import {validateBuilt} from './pack.mjs';

const bridge = new URL(
  '../schemas/web-plugin-bridge.schema.json',
  import.meta.url,
);

function sample(types = []) {
  if (!types.length) return {kind: 'empty', reason: 'no_object'};
  if (types.includes('core.text'))
    return {kind: 'text', source: 'selected_text', text: '{"sample":true}'};
  if (types.includes('core.url'))
    return {
      kind: 'url',
      source: 'selected_text',
      url: 'https://example.com/sample',
    };
  if (types.includes('core.file'))
    return {
      kind: 'resources',
      source: 'finder_selection',
      items: [
        {
          handle: 'mock-file',
          name: 'sample.csv',
          kind: 'file',
          readable: false,
          previewable: false,
        },
      ],
    };
  return {
    typeId: types[0],
    kind: 'data',
    value: {kind: 'object', count: 1, text: '{"sample":true}'},
  };
}

export async function testProject(project, manifest) {
  await validateBuilt(project, manifest);
  if (!manifest.background) {
    process.stdout.write(`Built Web entry verified for ${manifest.id}\n`);
    return;
  }
  execFileSync(
    process.execPath,
    [
      '--experimental-vm-modules',
      '--max-old-space-size=128',
      fileURLToPath(import.meta.url),
      project,
    ],
    {timeout: 10_000, stdio: 'inherit'},
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const project = process.argv[2];
    const {readManifest} = await import('./manifest.mjs');
    const {runBackgroundMock} = await import('./background-mock.mjs');
    const manifest = await readManifest(project);
    const source = await readFile(
      join(project, manifest.background.entry),
      'utf8',
    );
    const schema = await readFile(bridge, 'utf8').catch(error => {
      if (error.code !== 'ENOENT') throw error;
      return readFile(
        new URL(
          '../../../resources/web-plugin-bridge.schema.json',
          import.meta.url,
        ),
        'utf8',
      );
    });
    const methods = JSON.parse(schema).methods;
    const permissions = manifest.permissions.map(permission => permission.name);
    const resolve = (definition, role) =>
      runBackgroundMock({
        source,
        entrypoint: definition.entrypoint,
        role,
        context: sample(definition.accepts),
        invocation:
          role === 'resolver'
            ? null
            : {
                pluginId: manifest.id,
                commandId: definition.id,
                entrypoint: definition.entrypoint,
                input: sample(definition.accepts),
                capabilities: {
                  urlOpen: permissions.includes('url.open'),
                  resourceRead:
                    permissions.includes('resources.read') &&
                    definition.accepts?.includes('core.file') === true,
                  previewOpen: false,
                  windowControl: false,
                },
              },
        permissions,
        methods,
      });
    for (const resolver of manifest.background.resolvers ?? []) {
      const {value, denied} = await resolve(resolver, 'resolver');
      check(
        !denied.length,
        `Resolver ${resolver.id} attempted denied calls: ${denied.join(', ')}`,
      );
      check(
        Array.isArray(value) &&
          value.length <= 16 &&
          value.every(
            item =>
              item &&
              typeof item === 'object' &&
              Object.keys(item).length === 2 &&
              resolver.produces.includes(item.typeId) &&
              Object.hasOwn(item, 'value'),
          ),
        `Invalid resolver result: ${resolver.id}`,
      );
    }
    for (const command of manifest.commands ?? []) {
      if (command.runtime !== 'background') continue;
      const {value, denied} = await resolve(command, 'background');
      check(
        !denied.length,
        `Action ${command.id} attempted denied calls: ${denied.join(', ')}`,
      );
      check(
        value === null ||
          (value &&
            typeof value === 'object' &&
            !Array.isArray(value) &&
            Object.keys(value).every(
              key => key === 'message' || key === 'closeWindow',
            ) &&
            (!Object.hasOwn(value, 'message') ||
              (typeof value.message === 'string' &&
                Buffer.byteLength(value.message) <= 512)) &&
            (!Object.hasOwn(value, 'closeWindow') ||
              typeof value.closeWindow === 'boolean')),
        `Invalid background return: ${command.id}`,
      );
      if (value?.message?.trim() && value.closeWindow === true) {
        process.stderr.write(
          `Warning: ${command.id} closes the window before its message can be read.\n`,
        );
      }
    }
    process.stdout.write(`Background Mock passed for ${manifest.id}\n`);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
