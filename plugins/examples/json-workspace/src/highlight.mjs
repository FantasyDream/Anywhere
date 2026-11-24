import {createScanner, SyntaxKind} from 'jsonc-parser';

const MAX_HIGHLIGHT_LENGTH = 64 * 1024;
const MAX_HIGHLIGHT_TOKENS = 4096;
const colors = new Map([
  [SyntaxKind.StringLiteral, 'string'],
  [SyntaxKind.NumericLiteral, 'number'],
  [SyntaxKind.TrueKeyword, 'literal'],
  [SyntaxKind.FalseKeyword, 'literal'],
  [SyntaxKind.NullKeyword, 'literal'],
]);

/** Return spans over the exact editor text, including incomplete JSON. */
export function highlightJson(text) {
  if (!text || text.length > MAX_HIGHLIGHT_LENGTH) return null;
  const scanner = createScanner(text, true);
  const spans = [];
  let previous;
  let count = 0;
  for (
    let kind = scanner.scan();
    kind !== SyntaxKind.EOF;
    kind = scanner.scan()
  ) {
    if (++count > MAX_HIGHLIGHT_TOKENS) return null;
    if (kind === SyntaxKind.ColonToken && previous?.color === 'string') {
      previous.color = 'key';
    }
    const color = colors.get(kind);
    previous = undefined;
    if (color) {
      const start = scanner.getTokenOffset();
      previous = {start, end: start + scanner.getTokenLength(), color};
      spans.push(previous);
    }
  }
  return spans.length ? spans : null;
}
