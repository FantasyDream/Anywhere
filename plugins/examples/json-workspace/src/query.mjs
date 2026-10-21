import {parseTree, createScanner, SyntaxKind} from 'jsonc-parser';
import {JSONPathJS} from 'jsonpath-js';
import {validate, transform, MAX_BYTES} from './json.mjs';
import {calculate} from './javascript.mjs';
import {QueryError, MAX_QUERY_LENGTH, formula} from './expression.mjs';
export {QueryError, MAX_QUERY_LENGTH} from './expression.mjs';

const encoder = new TextEncoder();
const MAX_VALUES = 50000, MAX_MATCHES = 10000, MAX_VISITS = 200000;

// Detect decimal spellings that would change when converted to a JS number.
function decimalKey(text) {
  const [, sign, integer, fraction = '', exponent = '0'] = text.match(/^(-?)(\d+)(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/);
  let digits = (integer + fraction).replace(/^0+/, '');
  if (!digits) return '0';
  let power = Number(exponent) - fraction.length;
  const zeros = digits.match(/0+$/)?.[0].length ?? 0;
  digits = digits.slice(0, digits.length - zeros); power += zeros;
  return `${sign}${digits}e${power}`;
}
function exactNumber(raw) {
  const number = Number(raw);
  return Number.isFinite(number) && decimalKey(raw) === decimalKey(String(number));
}

function prepare(text) {
  validate(text, {maxNodes: MAX_VALUES});
  const root = parseTree(text), fields = new WeakMap();
  let imprecise = false;
  function value(node) {
    if (node.type === 'array') return node.children.map(value);
    if (node.type === 'object') {
      const object = Object.create(null), index = new Map();
      for (const property of node.children) {
        const [key, child] = property.children;
        object[key.value] = value(child); index.set(key.value, child);
      }
      fields.set(node, index); return object;
    }
    if (node.type === 'number' && !exactNumber(text.slice(node.offset, node.offset + node.length))) imprecise = true;
    return node.value;
  }
  const data = value(root);
  function fragment(segments) {
    let node = root;
    for (const segment of segments) node = node.type === 'array' ? node.children[segment] : fields.get(node)?.get(segment);
    if (!node) throw new QueryError('The selected value is no longer available');
    return text.slice(node.offset, node.offset + node.length);
  }
  return {data, fragment, imprecise};
}

function queryPolicy(query, document) {
  let nodes = 0, numericComparison = false;
  function inspect(ast) {
    if (!ast || typeof ast !== 'object') return;
    if (++nodes > 256) throw new QueryError('JSONPath is too complex');
    if (ast.type === 'FunctionExpr' && ['match', 'search'].includes(ast.name)) {
      throw new QueryError('Regular-expression filters are not supported');
    }
    if (ast.type === 'ComparisonExpr') {
      const operands = [ast.left, ast.right];
      numericComparison ||= operands.some(item => item.type === 'Literal' && typeof item.member === 'number')
        || operands.every(item => item.type !== 'Literal');
    }
    for (const child of Object.values(ast)) inspect(child);
  }
  inspect(query.rootNode);
  if (document.imprecise && numericComparison) {
    throw new QueryError('Numeric filters would lose precision. Select these values by path instead.');
  }
}

function checkLiterals(path) {
  // Mask quoted names/strings before checking numeric literals and indices.
  let plain = '', quote = '', escaped = false;
  for (const char of path) {
    if (quote) {
      plain += ' ';
      if (escaped) escaped = false;
      else if (char === '\\') escaped = true;
      else if (char === quote) quote = '';
    } else if (char === '"' || char === "'") { quote = char; plain += ' '; }
    else plain += char;
  }
  for (const token of plain.matchAll(/(^|[^\w$.])(-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)(?=$|[^\w.])/g)) {
    if (!exactNumber(token[2])) throw new QueryError('JSONPath numeric literal exceeds numeric precision');
  }
}

function bounded(data) {
  let visits = 0;
  const proxies = new WeakMap();
  function spend(count = 1) { if ((visits += count) > MAX_VISITS) throw new QueryError('Query is too expensive. Use a more specific path.'); }
  function wrap(value) {
    if (value === null || typeof value !== 'object') return value;
    if (!proxies.has(value)) proxies.set(value, new Proxy(value, {
      get(target, key, receiver) { spend(); return wrap(Reflect.get(target, key, receiver)); },
      has(target, key) { spend(); return Reflect.has(target, key); },
      ownKeys(target) { const keys = Reflect.ownKeys(target); spend(keys.length); return keys; },
    }));
    return proxies.get(value);
  }
  return wrap(data);
}

/** One document and one compiled query are retained, never a growing query cache. */
export class QueryEngine {
  #text; #document; #path; #query; #formatted; #spaces;
  clear() { this.#text = this.#document = this.#path = this.#query = this.#formatted = this.#spaces = undefined; }
  // Editing only the formula must not validate/format the entire JSON again.
  // Retain one successful formatted value, never the editor's caret or history.
  format(text, spaces = 2, selection) {
    if (text === this.#formatted && spaces === this.#spaces) return {text, selection};
    this.#formatted = this.#spaces = undefined;
    const result = autoFormat(text, spaces, selection);
    this.#formatted = result.text; this.#spaces = spaces;
    return result;
  }
  evaluate(text, path = '', spaces = 2, mode = 'jsonpath') {
    if (!['jsonpath','javascript'].includes(mode)) throw new QueryError('Unknown formula mode');
    path = formula(path, mode);
    if (path.length > MAX_QUERY_LENGTH) throw new QueryError('JSONPath exceeds 1,024 characters');
    if (text !== this.#text) {
      this.#text = this.#document = undefined;
      const document = prepare(text);
      this.#text = text; this.#document = document;
    }
    if (mode === 'javascript') return calculate(this.#document, path, spaces);
    if (path !== this.#path) {
      this.#path = this.#query = undefined;
      checkLiterals(path);
      let query;
      try { query = new JSONPathJS(path); } catch { throw new QueryError('Invalid JSONPath expression'); }
      this.#path = path; this.#query = query;
    }
    queryPolicy(this.#query, this.#document);
    const matches = this.#query.pathSegments(bounded(this.#document.data));
    if (matches.length > MAX_MATCHES) throw new QueryError('More than 10,000 matches. Use a more specific path.');
    let bytes = 2;
    const fragments = matches.map(({segments}) => {
      const fragment = this.#document.fragment(segments);
      bytes += encoder.encode(fragment).length + 1;
      if (bytes > MAX_BYTES) throw new QueryError('Query result exceeds 1 MiB');
      return fragment;
    });
    const raw = fragments.length === 1 ? fragments[0] : `[${fragments.join(',')}]`;
    return {output: transform(raw, 'format', spaces), count: matches.length};
  }
}

function tokens(text) {
  const scanner = createScanner(text, true), result = [];
  for (let kind = scanner.scan(); kind !== SyntaxKind.EOF; kind = scanner.scan()) {
    result.push({start: scanner.getTokenOffset(), length: scanner.getTokenLength()});
  }
  return result;
}
/** Keep selection anchored to the same JSON token when whitespace changes. */
export function autoFormat(text, spaces = 2, selection) {
  const formatted = transform(text, 'format', spaces);
  if (!selection || formatted === text) return {text: formatted, selection};
  const before = tokens(text), after = tokens(formatted);
  function move(offset) {
    if (offset >= text.length) return formatted.length;
    const index = before.findIndex(token => offset <= token.start + token.length);
    if (index < 0) return formatted.length;
    return after[index].start + Math.max(0, offset - before[index].start);
  }
  return {text: formatted, selection: {...selection, start: move(selection.start), end: move(selection.end)}};
}
