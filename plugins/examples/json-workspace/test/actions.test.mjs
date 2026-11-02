import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {
  JSON_ACTIONS,
  decodeDocument,
  prepareJsonAction,
  prepareJsonLaunch,
} from '../src/actions.mjs';
import {MAX_BYTES, transform} from '../src/json.mjs';

const source =
  '{"n":900719925474099312345,"n":1e9999,"s":"line\\n\\"quote\\"","items":[1,2]}';
test('copy modes preserve source tokens and escaping round-trips without evaluating JSON', () => {
  const pretty = prepareJsonAction(source, 'copy-format', 4);
  assert.equal(pretty.text, transform(source, 'format', 4));
  assert.equal(prepareJsonAction(pretty.text, 'copy-minify').text, source);
  const escaped = prepareJsonAction(source, 'copy-escaped');
  assert.equal(JSON.parse(escaped.text), source);
  assert.equal(decodeDocument(escaped.text).text, source);
  assert.equal(prepareJsonAction(source, 'copy-minify').text, source);
});

test('quoted, bare and nested escaped objects/arrays decode; normal string values are untouched', () => {
  let encoded = source;
  for (let i = 0; i < 4; i++) {
    encoded = JSON.stringify(encoded);
    assert.deepEqual(decodeDocument(encoded), {text: source, escaped: true});
    assert.deepEqual(decodeDocument(encoded.slice(1, -1)), {
      text: source,
      escaped: true,
    });
    assert.equal(
      prepareJsonAction(decodeDocument(encoded).text, 'copy-minify').text,
      source,
    );
  }
  assert.deepEqual(decodeDocument(source), {text: source, escaped: false});
  assert.deepEqual(decodeDocument(JSON.stringify('[true,null,"中文"]')), {
    text: '[true,null,"中文"]',
    escaped: true,
  });
  assert.equal(decodeDocument(JSON.stringify(encoded)).escaped, false); // fifth layer stays a JSON string
});

test('copy escaping supports scalar roots without reinterpreting ordinary scalars', () => {
  for (const text of ['null', 'true', '1e+30', '"hello"', '"\\u0041"']) {
    assert.equal(decodeDocument(text).text, text);
    assert.equal(
      JSON.parse(prepareJsonAction(text, 'copy-escaped').text),
      transform(text, 'minify'),
    );
  }
});

test('invalid input, inner JSON, action names and oversized output fail without partial copies', () => {
  for (const text of [
    '',
    '{bad}',
    String.raw`{\"n\":1,}`,
    String.raw`{\"n\":\x41}`,
  ]) {
    assert.throws(() => prepareJsonAction(text, 'copy-minify'));
  }
  for (const id of ['unknown', 'unescape'])
    assert.throws(() => prepareJsonAction('{}', id), /Unknown JSON action/);
  assert.throws(
    () => prepareJsonAction('x'.repeat(MAX_BYTES + 1), 'copy-format'),
    /1 MiB/,
  );
  const dense = '[' + Array(140000).fill('"\\n"').join(',') + ']';
  assert.throws(() => prepareJsonAction(dense, 'copy-escaped'), /1 MiB/);
  assert.throws(
    () =>
      prepareJsonAction('['.repeat(129) + '0' + ']'.repeat(129), 'copy-format'),
    /128/,
  );
});

test('launcher routes reuse the same actions, preserve input and never read clipboard implicitly', () => {
  const invocation = {
    input: {kind: 'text', text: JSON.stringify(source)},
    entrypoint: 'workspace',
  };
  assert.deepEqual(prepareJsonLaunch(invocation), {
    input: source,
    message: 'decoded',
  });
  for (const action of JSON_ACTIONS) {
    assert.equal(
      prepareJsonLaunch({
        input: {kind: 'empty', reason: 'no_object'},
        entrypoint: action.id,
      }),
      null,
    );
  }
  for (const id of ['copy-format', 'copy-minify', 'copy-escaped', 'unescape'])
    assert.throws(
      () => prepareJsonLaunch({...invocation, entrypoint: id}),
      /Unknown workspace/,
    );
  const manifest = JSON.parse(
    readFileSync(new URL('../manifest.json', import.meta.url)),
  );
  assert.ok(
    !manifest.permissions.some(
      permission => permission.name === 'clipboard.readText',
    ),
    'native editor paste must not require background clipboard access',
  );
  assert.deepEqual(
    manifest.commands.map(command => command.id),
    ['open', ...JSON_ACTIONS.map(action => action.id)],
  );
  const open = manifest.commands.find(command => command.id === 'open');
  assert.deepEqual(open.accepts, ['plugin.dev.anywhere.json-workspace.json']);
  assert.equal(open.inputMatchers, undefined);
  assert.deepEqual(manifest.background.resolvers[0].produces, open.accepts);
  assert.deepEqual(manifest.permissions[0].types, [
    'core.text',
    ...open.accepts,
  ]);
  for (const command of manifest.commands)
    assert.deepEqual(command.accepts, open.accepts);

  for (const action of JSON_ACTIONS) {
    const command = manifest.commands.find(command => command.id === action.id);
    assert.equal(command.requireContext, true);
    assert.equal(command.inputMatchers, undefined);
    assert.equal(command.runtime, 'background');
  }
});
