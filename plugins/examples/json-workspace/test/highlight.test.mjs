import assert from 'node:assert/strict';
import {test} from 'node:test';
import {highlightJson} from '../src/highlight.mjs';

function coloredText(text) {
  return highlightJson(text).map(({start, end, color}) => [
    text.slice(start, end),
    color,
  ]);
}

test('highlighting preserves exact token spelling and distinguishes JSON keys', () => {
  const text = String.raw`{"中文🚀" : "<b>\"quoted\"</b>", "amount": -2.3e+14, "list":[true,false,null]}`;
  assert.deepEqual(coloredText(text), [
    ['"中文🚀"', 'key'],
    [String.raw`"<b>\"quoted\"</b>"`, 'string'],
    ['"amount"', 'key'],
    ['-2.3e+14', 'number'],
    ['"list"', 'key'],
    ['true', 'literal'],
    ['false', 'literal'],
    ['null', 'literal'],
  ]);
});

test('incomplete input still gets coloring and large documents stay plain', () => {
  assert.deepEqual(coloredText('{"name": "中'), [
    ['"name"', 'key'],
    ['"中', 'string'],
  ]);
  assert.equal(highlightJson(' '), null);
  assert.equal(highlightJson(JSON.stringify('a'.repeat(65_536))), null);
  assert.equal(highlightJson(JSON.stringify(Array(4096).fill(0))), null);
});
