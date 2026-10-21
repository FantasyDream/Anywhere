import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {JSON_ACTIONS, decodeDocument, prepareJsonAction, prepareJsonLaunch} from '../src/actions.mjs';
import {MAX_BYTES, transform} from '../src/json.mjs';

const source = '{"n":900719925474099312345,"n":1e9999,"s":"line\\n\\\"quote\\\"","items":[1,2]}';
test('copy modes preserve source tokens and escaping round-trips without evaluating JSON', () => {
  const pretty = prepareJsonAction(source, 'copy-format', 4);
  assert.equal(pretty.text, transform(source, 'format', 4));
  assert.equal(pretty.copy, true);
  assert.equal(prepareJsonAction(pretty.text, 'copy-minify').text, source);
  const escaped = prepareJsonAction(source, 'copy-escaped');
  assert.equal(JSON.parse(escaped.text), source);
  assert.equal(prepareJsonAction(escaped.text, 'unescape').text, source);
  assert.equal(prepareJsonAction(escaped.text, 'unescape').copy, false);
  assert.equal(prepareJsonAction(source, 'copy-minify').text, source);
});

test('quoted, bare and nested escaped objects/arrays decode; normal string values are untouched', () => {
  let encoded = source;
  for (let i = 0; i < 4; i++) {
    encoded = JSON.stringify(encoded);
    assert.deepEqual(decodeDocument(encoded), {text: source, escaped: true});
    assert.deepEqual(decodeDocument(encoded.slice(1, -1)), {text: source, escaped: true});
    assert.equal(prepareJsonAction(encoded, 'copy-minify').text, source);
  }
  assert.deepEqual(decodeDocument(source), {text:source, escaped:false});
  assert.deepEqual(decodeDocument(JSON.stringify('[true,null,"中文"]')), {text:'[true,null,"中文"]', escaped:true});
  assert.equal(decodeDocument(JSON.stringify(encoded)).escaped, false); // fifth layer stays a JSON string
});

test('explicit escape/unescape also round-trips scalar roots without changing ordinary scalars on open', () => {
  for (const text of ['null', 'true', '1e+30', '"hello"', '"\\u0041"']) {
    assert.equal(decodeDocument(text).text, text);
    assert.equal(prepareJsonAction(prepareJsonAction(text, 'copy-escaped').text, 'unescape').text, text);
  }
});

test('invalid input, inner JSON, action names and oversized output fail without partial copies', () => {
  for (const text of ['', '{bad}', String.raw`{\"n\":1,}`, String.raw`{\"n\":\x41}`, '"hello"']) {
    assert.throws(() => prepareJsonAction(text, 'unescape'));
  }
  assert.throws(() => prepareJsonAction('{}', 'unknown'));
  assert.throws(() => prepareJsonAction('x'.repeat(MAX_BYTES + 1), 'copy-format'), /1 MiB/);
  const dense = '[' + Array(140000).fill('"\\n"').join(',') + ']';
  assert.throws(() => prepareJsonAction(dense, 'copy-escaped'), /1 MiB/);
  assert.throws(() => prepareJsonAction('['.repeat(129) + '0' + ']'.repeat(129), 'copy-format'), /128/);
});

test('launcher routes reuse the same actions, preserve input and never read clipboard implicitly', () => {
  const invocation = {input:{kind:'text', text:JSON.stringify(source)}, entrypoint:'workspace'};
  assert.deepEqual(prepareJsonLaunch(invocation), {input:source, clipboard:null, message:'decoded'});
  for (const action of JSON_ACTIONS) {
    const result = prepareJsonLaunch({...invocation, entrypoint:action.id});
    assert.equal(result.input, source);
    assert.equal(result.clipboard, action.id === 'unescape' ? null : prepareJsonAction(source, action.id).text);
    assert.equal(prepareJsonLaunch({input:{kind:'empty', reason:'no_object'}, entrypoint:action.id}), null);
  }
  const manifest = JSON.parse(readFileSync(new URL('../manifest.json', import.meta.url)));
  for (const action of JSON_ACTIONS) {
    const command = manifest.commands.find(command => command.id === action.id);
    assert.equal(command.entrypoint, action.id);
    assert.ok(command.inputMatchers.includes('json_escaped'));
  }
  assert.deepEqual(manifest.commands.find(command => command.id === 'unescape').inputMatchers, ['json_escaped']);
});
