import {decodeDocument} from './actions.mjs';
import {utf8Length} from './text.mjs';
import {JSON_CONTEXT} from './input.mjs';
export {JSON_CONTEXT} from './input.mjs';
export const MAX_RECOGNITION_BYTES = 16 * 1024;

/** Normalize outer escaping while retaining JSON tokens, never JS numeric values. */
export function resolveJsonContexts(input) {
  if (input.kind !== 'text' || utf8Length(input.text) > MAX_RECOGNITION_BYTES)
    return [];
  try {
    const decoded = decodeDocument(input.text);
    if (!['{', '['].includes(decoded.text.trimStart()[0])) return [];
    return [{type: JSON_CONTEXT, value: {text: decoded.text}}];
  } catch {
    return []; // An ordinary search term is not a plugin failure.
  }
}
