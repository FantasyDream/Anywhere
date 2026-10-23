import test from 'node:test';
import assert from 'node:assert/strict';
import {QueryEngine} from '../src/query.mjs';
import {formula, stripRoot} from '../src/expression.mjs';

const input =
  '{"users":[{"name":"Ada","age":30},{"name":"林","age":20}],"nil":null}';
function js(text, expression) {
  return new QueryEngine().evaluate(text, expression, 2, 'javascript').output;
}

test('fixed prefixes accept suffixes and pasted full formulas without duplication', () => {
  for (const [mode, prefix] of [
    ['jsonpath', '$'],
    ['javascript', 'this'],
  ]) {
    for (const value of ['.users', 'users', `${prefix}.users`])
      assert.equal(formula(value, mode), prefix + '.users');
    assert.equal(formula('', mode), prefix);
    assert.equal(formula('[0]', mode), prefix + '[0]');
  }
  assert.equal(stripRoot('this.users'), '.users');
  assert.equal(stripRoot('$.users'), '.users');
  assert.equal(stripRoot('thisYear'), 'thisYear');
  assert.equal(formula(' * 2', 'javascript'), 'this* 2');
  assert.throws(() => formula('.'.repeat(1025)), /1,024/);
});

test('JavaScript computes map/filter/reduce, object results, arithmetic and strings', () => {
  assert.deepEqual(JSON.parse(js(input, '.users.map(x=>x.name)')), [
    'Ada',
    '林',
  ]);
  assert.deepEqual(
    JSON.parse(js(input, '.users.filter(x=>x.age>20).map(x=>({name:x.name}))')),
    [{name: 'Ada'}],
  );
  assert.equal(js(input, '.users.reduce((n,x)=>n+x.age,0)'), '50');
  assert.equal(js('12', '* 2 + Math.round(1.2)'), '25');
  assert.equal(js(input, '.users[0].name.toUpperCase()'), '"ADA"');
  assert.equal(
    js(input, '.users.map(x=>`${x.name}!`).join(",")'),
    '"Ada!,林!"',
  );
  assert.deepEqual(JSON.parse(js(input, '.users.map(x=>Object.keys(x))')), [
    ['name', 'age'],
    ['name', 'age'],
  ]);
  assert.equal(js(input, '.users?.[0]?.name ?? "missing"'), '"Ada"');
});

test('JavaScript calculation never mutates the source cached for JSONPath', () => {
  const engine = new QueryEngine();
  assert.equal(
    JSON.parse(
      engine.evaluate(
        input,
        '.users.sort((a,b)=>a.age-b.age)[0].name',
        2,
        'javascript',
      ).output,
    ),
    '林',
  );
  assert.equal(engine.evaluate(input, '.users[0].name').output, '"Ada"');
  assert.equal(
    engine.evaluate(input, '.users[0].name', 2, 'javascript').output,
    '"Ada"',
  );
});

test('null, false, zero, arrays and undefined are explicit calculation results', () => {
  for (const [expression, output] of [
    ['.nil', 'null'],
    ['.missing', 'undefined'],
    ['.users.length > 3', 'false'],
    ['.users.length - 2', '0'],
  ]) {
    assert.equal(js(input, expression), output);
  }
  assert.equal(js('[]', ''), '[]');
  assert.throws(() => js('1', '/ 0'), /JSON data/);
  assert.throws(() => js('[1]', '.map(x=>x=>x)'), /JSON data/);
});

test('calculations cannot reach page, Host, native eval, constructors or mutate globals', () => {
  for (const expression of [
    '.users.map(x=>window)',
    '.users.map(x=>globalThis)',
    '.users.map(x=>document)',
    '.users.map(x=>host)',
    '.users.map(x=>fetch("https://example.com"))',
    '.users.map.constructor("return window")()',
    '.users.map(x=>Function("return window")())',
    '.users.map(x=>eval("1+1"))',
    '.users.map(x=>__anywhereTransport)',
    '.users.map(x=>new Array(1e9))',
    '.users.map(x=>{while(true){}})',
    '.users.map(x=>x.age=0)',
    '.users.map(async x=>x)',
    '.users.map(x=>/a+/)',
    '.users.map(x=>1n << 1000000000n)',
    '.users.map(x=>({get x(){return 1}}))',
  ])
    assert.throws(() => js(input, expression), expression);
  assert.equal(js(input, '.users.length'), '2');
});

test('execution, output and source budgets fail clearly and recover', () => {
  assert.throws(
    () =>
      js(
        '[' + Array(12000).fill('1').join(',') + ']',
        '.map(x=>x+1).map(x=>x+1)',
      ),
    /quota/i,
  );
  const large = '{"s":"' + 'a'.repeat(600000) + '"}';
  assert.throws(() => js(large, '.s + this.s'), /1 MiB|quota/);
  assert.throws(
    () => js(large, '.s.repeat(1000000000)'),
    /not permitted|not allowed|access|Cannot/,
  );
  assert.throws(() => js(large, '.s && [this.s,this.s].join("")'), /1 MiB/);
  assert.throws(
    () => js('{"id":9007199254740993123}', '.id'),
    /numeric precision/,
  );
  assert.throws(() => js('{"n":1e9999}', '.n'), /numeric precision/);
  assert.throws(() => js(input, '.users; window'), /one JavaScript expression/);
  assert.throws(() => js(input, '.users.map('), /Invalid JavaScript/);
  assert.equal(js(input, '.users.length'), '2');
});
