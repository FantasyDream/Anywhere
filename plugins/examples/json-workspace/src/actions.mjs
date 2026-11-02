import {jsonInputText} from './input.mjs';
import {utf8Length} from './text.mjs';
import {validate, transform, MAX_BYTES, JsonToolError} from './json.mjs';

export const JSON_ACTIONS = [
  {
    id: 'copy-format',
    icon: 'format',
  },
  {
    id: 'copy-minify',
    icon: 'minify',
  },
  {
    id: 'copy-escaped',
    icon: 'escape',
  },
];

function bounded(text) {
  if (utf8Length(text) > MAX_BYTES)
    throw new JsonToolError('JSON text exceeds 1 MiB');
  return text;
}
function container(text) {
  if (!['{', '['].includes(text.trimStart()[0])) return false;
  try {
    validate(text);
    return true;
  } catch {
    return false;
  }
}
function decodeLayer(text) {
  const source = text.trim();
  try {
    const value = JSON.parse(source.startsWith('"') ? source : `"${source}"`);
    if (typeof value !== 'string' || value === source) throw new Error();
    return bounded(value);
  } catch {
    throw new JsonToolError('Enter a valid escaped JSON string');
  }
}
/** Decode only outer string encoding; never replace escapes inside JSON values.
 * Recognition / workspace opening accepts containers, and keeps scalar roots as-is.
 */
export function decodeDocument(text) {
  bounded(text);
  if (container(text)) return {text, escaped: false};
  let decoded = text;
  for (let layer = 0; layer < 4; layer++) {
    try {
      decoded = decodeLayer(decoded);
    } catch {
      break;
    }
    if (container(decoded)) return {text: decoded, escaped: true};
  }
  validate(text);
  return {text, escaped: false};
}

export function prepareJsonAction(text, id, spaces = 2) {
  if (!JSON_ACTIONS.some(action => action.id === id))
    throw new JsonToolError('Unknown JSON action');
  let result = transform(
    text,
    id === 'copy-format' ? 'format' : 'minify',
    spaces,
  );
  if (id === 'copy-escaped') result = bounded(JSON.stringify(result));
  return {
    text: result,
    message: {
      'copy-format': 'copiedFormat',
      'copy-minify': 'copiedMinify',
      'copy-escaped': 'copiedEscaped',
    }[id],
  };
}
/** The sole UI route consumes normalized Context; native paste also accepts escaping. */
export function prepareJsonLaunch(invocation) {
  const source = jsonInputText(invocation.input);
  if (source === null) return null;
  if (invocation.entrypoint !== 'workspace')
    throw new JsonToolError('Unknown workspace route');
  const document = decodeDocument(source);
  return {input: document.text, message: document.escaped ? 'decoded' : null};
}
