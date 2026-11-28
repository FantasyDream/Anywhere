import assert from 'node:assert/strict';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {mkdtemp, mkdir, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHost, environment} from '../dist/index.js';
import {createMock, mockFile} from '../dist/mock.js';
import {readManifest, parseUniqueJson} from '../tools/manifest.mjs';
import {pack} from '../tools/pack.mjs';

test('platform formatting and exact matching share the same shortcut', () => {
  const key = {
    key: 'c',
    metaKey: false,
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
  };
  try {
    globalThis.__anywherePlatform = 'windows';
    assert.equal(environment.formatShortcut('Mod+Shift+C'), 'Ctrl+Shift+C');
    assert.ok(environment.matchesShortcut(key, 'Mod+Shift+C'));
    for (const props of [
      {altKey: true},
      {shiftKey: false},
      {repeat: true},
      {isComposing: true},
      {keyCode: 229},
      {defaultPrevented: true},
      {getModifierState: () => true},
    ])
      assert.equal(
        environment.matchesShortcut({...key, ...props}, 'Mod+Shift+C'),
        false,
      );
    globalThis.__anywherePlatform = 'macos';
    assert.equal(environment.formatShortcut('Mod+Shift+C'), '⌘⇧C');
    assert.ok(
      environment.matchesShortcut(
        {...key, metaKey: true, ctrlKey: false},
        'Mod+Shift+C',
      ),
    );
    assert.ok(!environment.matchesShortcut(key, 'Mod+Shift+C'));
    assert.throws(() => environment.formatShortcut('Mod+Ctrl+K'), TypeError);
    assert.throws(() => environment.formatShortcut('Mod+Meta+K'), TypeError);
    assert.equal(environment.matchesShortcut({}, 'Mod+Shift+C'), false);
  } finally {
    delete globalThis.__anywherePlatform;
  }
});

test('browser mock exercises the same resource and URL calls through the SDK', async () => {
  const mock = createMock({input: mockFile});
  const client = createHost(mock.transport);
  try {
    const invocation = await client.invocation.get();
    assert.equal(invocation.input.kind, 'resources');
    const handle = invocation.input.items[0].handle;
    assert.equal((await client.resources.stat(handle)).name, 'sample.csv');
    assert.match(await client.resources.readText(handle), /name,age/);
    await assert.rejects(client.resources.readText(handle, {maxBytes: 3}), {
      code: 'TOO_LARGE',
    });
    await assert.rejects(client.resources.readText(handle, {maxBytes: 0}), {
      code: 'INVALID_REQUEST',
    });
    const large = 'x'.repeat(70_000);
    mock.setClipboard(large);
    assert.equal(await client.clipboard.readText(), large);
    await assert.rejects(client.url.open('javascript:alert(1)'), {
      code: 'INVALID_REQUEST',
    });
    mock.setScenario('cancel-confirmation');
    await assert.rejects(client.url.open('https://example.test/record'), {
      code: 'USER_CANCELLED',
    });
    mock.setScenario('revoked');
    await assert.rejects(client.preview.open(handle), {
      code: 'PERMISSION_DENIED',
    });
  } finally {
    client.dispose();
  }
});

test('pack normalizes the manifest and is deterministic when replacing an output', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'awplugin-cli-'));
  try {
    await mkdir(join(dir, 'dist'));
    const fixture = new URL(
      '../../../crates/anywhere-plugin/tests/fixtures/duplicate-localized-title.json',
      import.meta.url,
    );
    await assert.rejects(
      async () => parseUniqueJson(await readFile(fixture, 'utf8')),
      /Duplicate JSON property/,
    );
    const manifest = JSON.parse(
      await readFile(
        new URL('../templates/minimal/manifest.json', import.meta.url),
        'utf8',
      ),
    );
    manifest.$schema = 'editor-schema.json';
    await writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
    await writeFile(join(dir, 'dist/index.html'), '<p>sample</p>');
    const inspected = await readManifest(dir);
    const output = join(dir, 'package.awplugin');
    await pack(dir, inspected, output);
    const archive = await readFile(output);
    await pack(dir, inspected, output);
    assert.deepEqual(await readFile(output), archive);
    assert.equal(
      createHash('sha256').update(archive).digest('hex'),
      '3a7fb30678e35d86b0e217bbaba1ef2c9e2bdfe6ec780b96118ca00e976e4120',
    );
    assert.equal(archive.readUInt16LE(6), 0x0800);
    assert.equal(archive.readUInt16LE(28), 0);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});
