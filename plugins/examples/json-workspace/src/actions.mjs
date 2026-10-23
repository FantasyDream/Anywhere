import {validate, transform, MAX_BYTES, JsonToolError} from './json.mjs';

const encoder = new TextEncoder();
export const JSON_ACTIONS = [
  {
    id: 'copy-format',
    title: 'Copy Formatted JSON',
    label: 'Copy formatted',
    icon: 'format',
  },
  {
    id: 'copy-minify',
    title: 'Copy Minified JSON',
    label: 'Copy minified',
    icon: 'minify',
  },
  {
    id: 'copy-escaped',
    title: 'Copy Escaped JSON',
    label: 'Copy escaped',
    icon: 'escape',
  },
  {id: 'unescape', title: 'Unescape JSON', label: 'Unescape', icon: 'unescape'},
];

function bounded(text) {
  if (encoder.encode(text).length > MAX_BYTES)
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
  if (id === 'unescape') {
    // Containers support up to four layers. Explicit unescape also supports one
    // layer around a scalar, so escape/unescape round-trips every JSON root type.
    const document = decodeDocumentOrNull(text);
    const decoded = document?.escaped
      ? document.text
      : decodeLayer(bounded(text));
    validate(decoded);
    return {text: decoded, copy: false, message: 'unescaped'};
  }
  const source = decodeDocument(text).text;
  let result = transform(
    source,
    id === 'copy-format' ? 'format' : 'minify',
    spaces,
  );
  if (id === 'copy-escaped') result = bounded(JSON.stringify(result));
  return {
    text: result,
    copy: true,
    message: {
      'copy-format': 'copiedFormat',
      'copy-minify': 'copiedMinify',
      'copy-escaped': 'copiedEscaped',
    }[id],
  };
}
function decodeDocumentOrNull(text) {
  try {
    return decodeDocument(text);
  } catch {
    return null;
  }
}

/** The same ordinary Web page serves Workspace and all four launcher commands.
 * No input means open the editor, never implicitly read the clipboard.
 */
export function prepareJsonLaunch(invocation, spaces = 2) {
  if (invocation.input.kind !== 'text') return null;
  const source = invocation.input.text;
  if (invocation.entrypoint === 'workspace') {
    const document = decodeDocument(source);
    return {
      input: document.text,
      clipboard: null,
      message: document.escaped ? 'decoded' : null,
    };
  }
  const result = prepareJsonAction(source, invocation.entrypoint, spaces);
  return {
    input: result.copy ? decodeDocument(source).text : result.text,
    clipboard: result.copy ? result.text : null,
    message: result.message,
  };
}
