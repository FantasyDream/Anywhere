import test from 'node:test';
import assert from 'node:assert/strict';
import {
  resolveJsonContexts,
  JSON_CONTEXT,
  MAX_RECOGNITION_BYTES,
} from '../src/contexts.mjs';
import {jsonInputText} from '../src/input.mjs';
import {copyJson} from '../src/background-actions.mjs';
import {utf8Length} from '../src/text.mjs';
import {prepareJsonAction, prepareJsonLaunch} from '../src/actions.mjs';
import {workspaceInput} from '../src/workspace-input.mjs';

const source = '{"n":900719925474099312345,"n":1e9999,"s":"中文🚀\\n"}';
const input = text => ({kind: 'text', source: 'selected_text', text});
const data = value => ({kind: 'data', type_id: value.type, value: value.value});

test('UTF-8 budgets match Web encoding without requiring browser APIs', () => {
  for (const text of [
    '',
    'ASCII',
    '中文🚀',
    '\ud800',
    '\udc00',
    '\ud800x\udc00',
    '\ufeff\n',
  ])
    assert.equal(utf8Length(text), new TextEncoder().encode(text).length);
});

test('resolver recognizes containers and escaped containers without rounding source tokens', () => {
  const plain = resolveJsonContexts(input(source));
  assert.deepEqual(plain, [{type: JSON_CONTEXT, value: {text: source}}]);
  let encoded = source;
  for (let layer = 0; layer < 4; layer++) {
    encoded = JSON.stringify(encoded);
    const contexts = resolveJsonContexts(input(encoded));
    assert.deepEqual(contexts, plain);
    assert.deepEqual(resolveJsonContexts(input(encoded.slice(1, -1))), plain);
    assert.equal(jsonInputText(data(contexts[0])), source);
    assert.equal(
      prepareJsonLaunch({input: data(contexts[0]), entrypoint: 'workspace'})
        .input,
      source,
    );
  }
  for (const text of [
    'word',
    '123',
    'null',
    'true',
    '"hello"',
    '{broken}',
    '[1,]',
    JSON.stringify(encoded),
  ])
    assert.deepEqual(resolveJsonContexts(input(text)), []);
  assert.deepEqual(
    resolveJsonContexts({kind: 'empty', reason: 'no_object'}),
    [],
  );
  assert.deepEqual(
    resolveJsonContexts(input(' '.repeat(MAX_RECOGNITION_BYTES) + '{}')),
    [],
  );
});

test('background copies use the selected Context, saved indentation and installed clipboard grants', async () => {
  const writes = [],
    reads = [];
  const host = {
    context: {
      get: async () =>
        data(resolveJsonContexts(input(JSON.stringify(source)))[0]),
    },
    storage: {
      get: async key => {
        reads.push(key);
        return 4;
      },
    },
    clipboard: {
      readText: () => {
        throw Error('must not read clipboard');
      },
      writeText: async text => {
        writes.push(text);
      },
    },
  };
  for (const id of ['copy-format', 'copy-minify', 'copy-escaped']) {
    await copyJson(host, id);
    assert.equal(writes.at(-1), prepareJsonAction(source, id, 4).text);
  }
  assert.deepEqual(reads, ['indent']);
  host.clipboard.writeText = async () => {
    throw Object.assign(Error(), {code: 'USER_CANCELLED'});
  };
  await assert.rejects(copyJson(host, 'copy-minify'), {code: 'USER_CANCELLED'});
  assert.equal(writes.length, 3);
  host.context.get = async () => input(source);
  await assert.rejects(copyJson(host, 'copy-format'), /requires a recognized/);
  host.context.get = async () => ({
    kind: 'data',
    type_id: JSON_CONTEXT,
    value: {text: '{broken}'},
  });
  await assert.rejects(copyJson(host, 'copy-minify'));
  assert.equal(writes.length, 3);
});

test('optional unavailable preferences fall back, but revoked access cannot cause a copy', async () => {
  const writes = [];
  const host = {
    context: {get: async () => data(resolveJsonContexts(input('{}'))[0])},
    storage: {
      get: async () => {
        throw {code: 'STORAGE_UNAVAILABLE'};
      },
    },
    clipboard: {writeText: async text => writes.push(text)},
  };
  await copyJson(host, 'copy-format');
  assert.deepEqual(writes, ['{}']);
  host.storage.get = async () => {
    throw {code: 'PERMISSION_DENIED'};
  };
  await assert.rejects(copyJson(host, 'copy-format'), {
    code: 'PERMISSION_DENIED',
  });
  assert.equal(writes.length, 1);
});

test('live Context follows launch ordering, deduplicates and never overwrites an edited or busy draft', () => {
  const loaded = [],
    notices = [],
    errors = [];
  let edited = false,
    busy = false;
  const flow = workspaceInput({
    canReplace: () => !edited && !busy,
    load: text => loaded.push(text),
    notice: key => notices.push(key),
    report: error => errors.push(error),
  });
  const normalized = data(
    resolveJsonContexts(input(JSON.stringify(source)))[0],
  );
  flow.context(normalized);
  flow.launch({input: normalized, entrypoint: 'workspace'});
  assert.deepEqual(loaded, [source]);
  assert.deepEqual(notices, []);
  flow.context(data(resolveJsonContexts(input('[1]'))[0]));
  assert.equal(loaded.at(-1), '[1]');
  edited = true;
  flow.context(input('[2]'));
  assert.equal(loaded.at(-1), '[1]');
  edited = false;
  busy = true;
  flow.context(input('[3]'));
  assert.equal(loaded.at(-1), '[1]');
  busy = false;
  flow.context({kind: 'empty', reason: 'no_object'});
  assert.equal(loaded.at(-1), '[1]');
  assert.deepEqual(errors, []);
});
