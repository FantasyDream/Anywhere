import {copyFile, mkdir} from 'node:fs/promises';

const target = new URL('../schemas/', import.meta.url);
await mkdir(target, {recursive: true});
for (const name of [
  'web-plugin-manifest.schema.json',
  'web-plugin-manifest-editor.schema.json',
  'web-plugin-bridge.schema.json',
]) {
  await copyFile(
    new URL(`../../../resources/${name}`, import.meta.url),
    new URL(name, target),
  );
}
