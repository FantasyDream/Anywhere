import {anywhere} from '@anywhere/sdk';

export async function resolve() {
  const input = await anywhere.context.get();
  if (input.kind !== 'text') return [];
  let value;
  try {
    value = JSON.parse(input.text);
  } catch {
    return [];
  }
  if (!value || typeof value !== 'object') return [];
  return [
    {
      type: 'plugin.dev.example.json-summary.summary',
      value: {
        kind: Array.isArray(value) ? 'array' : 'object',
        count: Object.keys(value).length,
      },
    },
  ];
}

export async function copy() {
  const input = await anywhere.context.get();
  if (input.kind !== 'data') return;
  await anywhere.clipboard.writeText(JSON.stringify(input.value, null, 2));
}
