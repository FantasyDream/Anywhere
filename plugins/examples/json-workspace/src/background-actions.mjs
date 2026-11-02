import {prepareJsonAction} from './actions.mjs';
import {JSON_CONTEXT, jsonInputText} from './input.mjs';

/** Explicit command only. The resolver never reads storage or touches the clipboard. */
export async function copyJson(anywhere, action) {
  const input = await anywhere.context.get();
  if (input.kind !== 'data' || input.type_id !== JSON_CONTEXT)
    throw new Error('This action requires a recognized JSON document');
  const source = jsonInputText(input);
  if (source === null) throw new Error('The JSON context has no source text');
  let spaces = 2;
  if (action === 'copy-format') {
    try {
      const stored = await anywhere.storage.get('indent');
      if ([2, 4].includes(stored)) spaces = stored;
    } catch (error) {
      if (error.code !== 'STORAGE_UNAVAILABLE') throw error;
    }
  }
  const result = prepareJsonAction(source, action, spaces);
  await anywhere.clipboard.writeText(result.text);
}
