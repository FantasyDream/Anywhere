import Sandbox from '@nyariv/sandboxjs';
import {parseExpressionAt} from 'acorn';
import {QueryError} from './expression.mjs';
import {MAX_BYTES, transform} from './json.mjs';

// A calculation expression, not a script/program. No loops, declarations,
// mutation, getters, async work or imports; arrows are for map/filter/reduce.
const allowed = new Set(['ThisExpression', 'Identifier', 'Literal', 'MemberExpression',
  'CallExpression', 'ArrowFunctionExpression', 'BinaryExpression', 'LogicalExpression',
  'UnaryExpression', 'ConditionalExpression', 'ArrayExpression', 'ObjectExpression',
  'Property', 'ChainExpression', 'TemplateLiteral', 'TemplateElement']);
const prototypes = new Map([
  [Array, new Set(['map','filter','reduce','reduceRight','find','findIndex','some','every',
    'slice','sort','reverse','join','includes','indexOf','lastIndexOf','at'])],
  [String, new Set(['includes','startsWith','endsWith','indexOf','lastIndexOf','slice',
    'substring','charAt','charCodeAt','toLowerCase','toUpperCase','trim','trimStart','trimEnd'])],
  [Number, new Set(['toFixed','toString','valueOf'])],
  [Boolean, new Set(['toString','valueOf'])],
  [Object, new Set(['hasOwnProperty'])],
]);
const globals = Object.freeze({
  Math, Number, String, Boolean, parseInt, parseFloat, isFinite, isNaN,
  Object: Object.freeze({keys: Object.keys, values: Object.values, entries: Object.entries}),
  undefined, NaN, Infinity,
});

// A native join can otherwise allocate a large string in a single interpreter step.
const replacements = new Map([[Array.prototype.join, (_context, original) => function(separator = ',') {
  if (typeof separator !== 'string') throw new QueryError('Join separator must be a string');
  const encoder = new TextEncoder();
  let bytes = Math.max(0, this.length - 1) * encoder.encode(separator).length;
  for (const value of this) {
    if (value != null && !['string','number','boolean'].includes(typeof value)) throw new QueryError('Join accepts primitive values only');
    bytes += encoder.encode(value == null ? '' : String(value)).length;
    if (bytes > MAX_BYTES) throw new QueryError('Calculation result exceeds 1 MiB');
  }
  return original.apply(this, [separator]);
}]]);

function compileSource(expression) {
  let ast;
  try { ast = parseExpressionAt(expression, 0, {ecmaVersion: 2022}); }
  catch { throw new QueryError('Invalid JavaScript expression'); }
  if (expression.slice(ast.end).trim()) throw new QueryError('Enter one JavaScript expression');
  let nodes = 0;
  const roots = [];
  function inspect(node, depth = 0) {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) { for (const child of node) inspect(child, depth); return; }
    if (!node.type) return;
    if (++nodes > 256 || depth > 48) throw new QueryError('JavaScript expression is too complex');
    if (!allowed.has(node.type) || node.async || node.generator || node.regex || node.bigint
        || node.type === 'Property' && (node.kind !== 'init' || node.method)
        || node.type === 'UnaryExpression' && node.operator === 'delete') {
      throw new QueryError('Use a synchronous calculation expression; scripts and mutation are not supported');
    }
    if (node.type === 'ThisExpression') roots.push(node);
    if (node.type === 'ArrowFunctionExpression' && node.params.some(p => p.type !== 'Identifier')) {
      throw new QueryError('Use named arrow parameters, for example x => x.name');
    }
    for (const value of Object.values(node)) if (value && typeof value === 'object') inspect(value, depth + 1);
  }
  inspect(ast);
  // The interpreter gets a detached JSON value, never the page's this/window.
  let source = expression;
  for (const node of roots.reverse()) source = source.slice(0, node.start) + '__json' + source.slice(node.end);
  return `return (${source});`;
}

function clone(value) {
  if (Array.isArray(value)) return value.map(clone);
  if (value && typeof value === 'object') {
    const copy = Object.create(null);
    for (const [key, child] of Object.entries(value)) copy[key] = clone(child);
    return copy;
  }
  return value;
}

/** Serialize only data, never invoking a user getter, toJSON or conversion hook. */
function resultText(value, spaces) {
  if (value === undefined) return 'undefined';
  const parts = [], active = new Set(), encoder = new TextEncoder();
  let bytes = 0, nodes = 0;
  function append(text) {
    bytes += encoder.encode(text).length;
    if (bytes > MAX_BYTES) throw new QueryError('Calculation result exceeds 1 MiB');
    parts.push(text);
  }
  function write(item, depth = 0) {
    if (++nodes > 50000 || depth > 128) throw new QueryError('Calculation result is too complex');
    if (item === null || ['string','boolean'].includes(typeof item)) { append(JSON.stringify(item)); return; }
    if (typeof item === 'number' && Number.isFinite(item)) { append(JSON.stringify(item)); return; }
    if (typeof item !== 'object') throw new QueryError('Calculation must return JSON data or undefined');
    if (active.has(item)) throw new QueryError('Calculation returned a circular value');
    active.add(item);
    const array = Array.isArray(item);
    if (array && item.length > 50000) throw new QueryError('Calculation result is too complex');
    append(array ? '[' : '{');
    const descriptors = Object.getOwnPropertyDescriptors(item);
    const keys = array ? Array.from({length:item.length}, (_, i) => String(i))
      : Object.keys(descriptors).filter(key => descriptors[key].enumerable);
    let emitted = 0;
    for (const key of keys) {
      const descriptor = descriptors[key];
      if (descriptor && !Object.hasOwn(descriptor, 'value')) throw new QueryError('Computed accessors are not supported');
      const child = descriptor?.value;
      if (!array && child === undefined) continue;
      if (emitted++) append(',');
      if (!array) append(JSON.stringify(key) + ':');
      write(child === undefined ? null : child, depth + 1);
    }
    append(array ? ']' : '}'); active.delete(item);
  }
  write(value);
  return transform(parts.join(''), 'format', spaces);
}

export function calculate(document, expression, spaces = 2) {
  if (document.imprecise) throw new QueryError('JavaScript would lose numeric precision. Use JSONPath for this document.');
  const source = compileSource(expression);
  const sandbox = new Sandbox({globals, prototypeWhitelist: prototypes, symbolWhitelist: {},
    executionQuota: 20000n, maxParserRecursionDepth: 64, haltOnSandboxError: false,
    functionReplacements: replacements});
  try {
    // Eagerly parse nested arrows too: lazy function parsing does not share the
    // same operation budget. Disable dynamic compilation even outside Host CSP.
    const execution = sandbox.compile(source, true)({__json: clone(document.data)});
    execution.context.allowJit = false;
    const value = execution.run();
    return {output: resultText(value, spaces), count: 1};
  } catch (error) {
    if (error instanceof QueryError) throw error;
    throw new QueryError(`JavaScript: ${error.message}`);
  }
}
