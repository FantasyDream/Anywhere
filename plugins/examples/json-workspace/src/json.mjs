import {
  createScanner,
  visit,
  SyntaxKind,
  printParseErrorCode,
} from 'jsonc-parser';
export const MAX_BYTES = 1024 * 1024;
const encoder = new TextEncoder();
export class JsonToolError extends Error {
  constructor(message, offset = 0, text = '') {
    const before = text.slice(0, offset).split('\n');
    const line = before.length,
      column = before.at(-1).length + 1;
    super(`${message} · line ${line}, column ${column}`);
    this.offset = offset;
    this.line = line;
    this.column = column;
  }
}
export function validate(text, {maxNodes = Infinity} = {}) {
  if (!text.trim()) throw new JsonToolError('Enter JSON to begin');
  if (encoder.encode(text).length > MAX_BYTES)
    throw new JsonToolError('Input exceeds 1 MiB');
  const scanner = createScanner(text, true);
  let depth = 0;
  for (
    let token = scanner.scan();
    token !== SyntaxKind.EOF;
    token = scanner.scan()
  ) {
    if (
      token === SyntaxKind.OpenBraceToken ||
      token === SyntaxKind.OpenBracketToken
    ) {
      if (++depth > 128)
        throw new JsonToolError(
          'Nesting exceeds 128 levels',
          scanner.getTokenOffset(),
          text,
        );
    } else if (
      token === SyntaxKind.CloseBraceToken ||
      token === SyntaxKind.CloseBracketToken
    )
      depth--;
  }
  let first,
    nodes = 0;
  const count = offset => {
    if (++nodes > maxNodes)
      throw new JsonToolError(
        'Document has too many values to query',
        offset,
        text,
      );
  };
  visit(
    text,
    {
      onObjectBegin: count,
      onArrayBegin: count,
      onLiteralValue: (_, offset) => count(offset),
      onError(code, offset) {
        first ??= new JsonToolError(printParseErrorCode(code), offset, text);
      },
    },
    {
      allowTrailingComma: false,
      disallowComments: true,
      allowEmptyContent: false,
    },
  );
  if (first) throw first;
}
/** Token-preserving formatting: large integers, exponents, escapes, duplicate keys
 * and ordering are never round-tripped through JS numbers or object properties. */
export function transform(text, mode = 'format', spaces = 2) {
  validate(text);
  if (mode === 'validate') return text;
  if (!['format', 'minify'].includes(mode) || ![2, 4].includes(spaces))
    throw new Error('Unsupported format');
  const scanner = createScanner(text, true);
  const parts = [];
  let bytes = 0,
    depth = 0,
    previous = SyntaxKind.EOF;
  const append = value => {
    bytes += encoder.encode(value).length;
    if (bytes > MAX_BYTES)
      throw new JsonToolError('Formatted output exceeds 1 MiB');
    parts.push(value);
  };
  const newline = () => append('\n' + ' '.repeat(depth * spaces));
  for (
    let token = scanner.scan();
    token !== SyntaxKind.EOF;
    token = scanner.scan()
  ) {
    const raw = text.slice(
      scanner.getTokenOffset(),
      scanner.getTokenOffset() + scanner.getTokenLength(),
    );
    if (mode === 'minify') {
      append(raw);
      continue;
    }
    const closing =
      token === SyntaxKind.CloseBraceToken ||
      token === SyntaxKind.CloseBracketToken;
    const opening =
      token === SyntaxKind.OpenBraceToken ||
      token === SyntaxKind.OpenBracketToken;
    const previousOpen =
      previous === SyntaxKind.OpenBraceToken ||
      previous === SyntaxKind.OpenBracketToken;
    if (closing) {
      depth--;
      if (!previousOpen) newline();
    } else if (previousOpen) newline();
    append(raw);
    if (opening) depth++;
    else if (token === SyntaxKind.CommaToken) newline();
    else if (token === SyntaxKind.ColonToken) append(' ');
    previous = token;
  }
  return parts.join('');
}
