export const JSON_CONTEXT = 'plugin.dev.anywhere.json-workspace.json';

export function jsonInputText(input) {
  if (input.kind === 'text') return input.text;
  if (
    input.kind === 'data' &&
    input.type_id === JSON_CONTEXT &&
    typeof input.value?.text === 'string'
  )
    return input.value.text;
  return null;
}
