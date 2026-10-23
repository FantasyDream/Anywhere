import test from 'node:test';
import assert from 'node:assert/strict';
import {QueryEngine, autoFormat, MAX_QUERY_LENGTH} from '../src/query.mjs';

test('JSONPath selects fields, indices, quoted names, wildcards, slices and descendants', () => {
  const engine = new QueryEngine();
  const text =
    '{"users":[{"name":"Ada","age":30},{"name":"林","age":20}],"a.b":{"😀":"ok"}}';
  for (const [path, value, count] of [
    ['$.users[0].name', 'Ada', 1],
    ['$.users[-1].age', 20, 1],
    ['$.users[*].name', ['Ada', '林'], 2],
    ['$..age', [30, 20], 2],
    ['$.users[0:1].name', 'Ada', 1],
    ["$['a.b']['😀']", 'ok', 1],
    ['$.missing', [], 0],
    ['$.users[?(@.age >= 25)].name', 'Ada', 1],
  ]) {
    const result = engine.evaluate(text, path);
    assert.equal(result.count, count, path);
    assert.deepEqual(JSON.parse(result.output), value, path);
  }
});

test('results preserve large numbers, exponents, escapes and duplicate-key fragments', () => {
  const engine = new QueryEngine();
  const input =
    '{"n":1,"n":900719925474099312345,"exp":1e9999,"s":"\\u0041","obj":{"k":1,"k":2}}';
  assert.equal(engine.evaluate(input, '$.n').output, '900719925474099312345');
  assert.equal(engine.evaluate(input, '$.exp').output, '1e9999');
  assert.equal(engine.evaluate(input, '$.s').output, '"\\u0041"');
  const root = engine.evaluate(input, '$').output;
  assert.equal((root.match(/"n":/g) ?? []).length, 2);
  assert.equal(
    (engine.evaluate(input, '$.obj').output.match(/"k":/g) ?? []).length,
    2,
  );
  assert.equal(
    engine.evaluate(
      '[{"id":900719925474099312345,"name":"Ada"}]',
      "$[?@.name == 'Ada'].id",
    ).output,
    '900719925474099312345',
  );
});

test('missing values, null, false, zero and empty arrays remain distinct', () => {
  const engine = new QueryEngine();
  const text = '{"nil":null,"no":false,"zero":0,"empty":[],"str":""}';
  for (const [path, output, count] of [
    ['$.nil', 'null', 1],
    ['$.no', 'false', 1],
    ['$.zero', '0', 1],
    ['$.empty', '[]', 1],
    ['$.str', '""', 1],
    ['$.absent', '[]', 0],
  ]) {
    assert.deepEqual(engine.evaluate(text, path), {output, count});
  }
  assert.equal(engine.evaluate('"scalar"', '  ').output, '"scalar"');
});

test('numeric filters reject precision loss instead of silently matching rounded IDs', () => {
  const engine = new QueryEngine();
  for (const number of ['9007199254740993', '1.0000000000000001', '1e9999']) {
    assert.throws(
      () => engine.evaluate(`[{"n":${number}}]`, '$[?@.n > 0]'),
      /precision/,
    );
  }
  for (const literal of ['9007199254740993', '0.10000000000000001', '1e9999']) {
    assert.throws(
      () => engine.evaluate('[{"n":1}]', `$[?@.n == ${literal}]`),
      /precision/,
    );
  }
  assert.equal(
    engine.evaluate('[{"n":0.1}]', '$[?@.n == 0.1].n').output,
    '0.1',
  );
});

test('queries cannot access inherited properties or execute scripts and regexes', () => {
  const engine = new QueryEngine();
  for (const path of ['$.constructor', '$.__proto__', '$.toString'])
    assert.equal(engine.evaluate('{}', path).count, 0);
  assert.equal(
    engine.evaluate('{"__proto__":{"safe":true}}', '$.__proto__.safe').output,
    'true',
  );
  for (const path of [
    '$[?match(@.name,"(a+)+")]',
    '$[?search(@,"a")]',
    '$[?(@.x.constructor())]',
    'not a path',
  ]) {
    assert.throws(() => engine.evaluate('[{"name":"a"}]', path));
  }
});

test('query and result budgets fail without truncation and a simpler query can recover', () => {
  const engine = new QueryEngine();
  assert.throws(
    () => engine.evaluate('{}', '$' + '.a'.repeat(MAX_QUERY_LENGTH)),
    /1,024/,
  );
  const bigArray = '[' + Array(10001).fill('0').join(',') + ']';
  assert.throws(() => engine.evaluate(bigArray, '$[*]'), /10,000 matches/);
  assert.equal(engine.evaluate(bigArray, '$[0]').output, '0');
  assert.throws(
    () => engine.evaluate('[' + Array(50001).fill('0').join(',') + ']'),
    /too many values/,
  );
  const largeValue = '["' + 'a'.repeat(600000) + '"]';
  assert.throws(() => engine.evaluate(largeValue, '$[0,0]'), /result exceeds/);
  const recursive = '{"a":'.repeat(80) + '0' + '}'.repeat(80);
  assert.throws(
    () => engine.evaluate(recursive, '$..*..*..*..*'),
    /too expensive/,
  );
  assert.equal(
    engine.evaluate('{"recovered":true}', '$.recovered').output,
    'true',
  );
  engine.clear();
  assert.equal(engine.evaluate('[1,2]', '$[1]').output, '2');
});

test('automatic formatting keeps caret and selection on the same JSON token', () => {
  const source = '{"name":"中文","count":123}';
  const start = source.indexOf('123');
  const formatted = autoFormat(source, 2, {
    start: start + 1,
    end: start + 3,
    direction: 'forward',
    top: 5,
    left: 0,
  });
  assert.equal(
    formatted.text.slice(formatted.selection.start, formatted.selection.end),
    '23',
  );
  assert.equal(formatted.selection.top, 5);
  assert.equal(
    autoFormat(source, 4, {start: source.length, end: source.length}).selection
      .start,
    autoFormat(source, 4).text.length,
  );
  assert.throws(() => autoFormat('{"typing":'), /line 1/);
  assert.equal(source, '{"name":"中文","count":123}');
});

test('formula-only edits preserve the current caret; content and indent still reformat', () => {
  const engine = new QueryEngine();
  const first = engine.format('{"number":900719925474099312345}', 2);
  const selection = {
    start: 8,
    end: 15,
    direction: 'backward',
    top: 60,
    left: 10,
  };
  assert.deepEqual(engine.format(first.text, 2, selection), {
    text: first.text,
    selection,
  });
  assert.equal(
    engine.evaluate(first.text, '.number').output,
    '900719925474099312345',
  );
  const indented = engine.format(first.text, 4, selection);
  assert.equal(indented.text, autoFormat(first.text, 4).text);
  assert.equal(indented.selection.top, selection.top);
  assert.equal(
    engine.format('{"changed":true}', 4).text,
    autoFormat('{"changed":true}', 4).text,
  );
  assert.throws(() => engine.format('{"typing":', 4), /line 1/);
  engine.clear();
  assert.equal(engine.format(first.text, 2, selection).text, first.text);
});
