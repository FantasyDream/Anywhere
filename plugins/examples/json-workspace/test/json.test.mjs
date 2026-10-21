import test from 'node:test';
import assert from 'node:assert/strict';
import {transform,validate} from '../src/json.mjs';
test('format/minify preserve integer precision, exponent, escapes, key order and duplicate keys',()=>{
 const input='{"n":900719925474099312345,"n":1e+30,"__proto__":{},"escape":"\\u0041","list":[true,null]}';
 const formatted=transform(input);
 assert.equal(transform(formatted,'minify'),input);
 assert.ok(formatted.includes('900719925474099312345'));
 assert.ok(formatted.includes('"n": 1e+30'));
 assert.equal(JSON.parse(formatted).list[1],null);
});
test('empty collections and all JSON scalar roots',()=>{
 for(const input of ['{}','[]','null','true','-1.50e-3','"你好"']) assert.equal(transform(transform(input),'minify'),input);
});
test('strict errors include correct location, comments/trailing commas rejected',()=>{
 for(const value of ['','{bad}','[1,]','{"a":1//comment\n}','{"a":01}','true false']) assert.throws(()=>validate(value));
 assert.throws(()=>validate('{\n"a":?}'),/line 2, column 5/);
});
test('input, output and nesting budgets reject without truncation',()=>{
 assert.throws(()=>validate('"'+'中'.repeat(400000)+'"'),/1 MiB/);
 assert.throws(()=>validate('['.repeat(129)+'0'+']'.repeat(129)),/128/);
 const dense='['.repeat(100)+'['+Array(12000).fill('0').join(',')+']'+']'.repeat(100);
 assert.throws(()=>transform(dense),/output exceeds/);
});
test('format is idempotent at either indent size',()=>{
 const input='{"a":[1,{"b":"x y"}],"c":{}}';
 for(const spaces of [2,4]){const out=transform(input,'format',spaces);assert.equal(transform(out,'format',spaces),out);}
});
