import {anywhere} from '../../dist/background.js';

export async function resolve() {
  const input = await anywhere.context.get();
  if (input.kind !== 'text') return [];
  const value = JSON.parse(input.text);
  return [
    {
      type: 'plugin.dev.example.json-summary.summary',
      value: {kind: 'object', count: Object.keys(value).length},
    },
  ];
}

export async function copy() {
  const input = await anywhere.context.get();
  if (input.kind !== 'data') throw new Error('Missing fixture context');
  // Run without TextEncoder/TextDecoder, through real QuickJS and the Rust broker.
  const samples = {
    small: () => '\ufeffsmall🚀',
    unicode: () => 'a'.repeat(4095) + '🚀\ufeff中é\u0000'.repeat(10000),
    limit: () => '中🚀'.repeat(149796) + 'test',
    escaped: () => '\u0001'.repeat(20000),
  };
  if (globalThis.__textUploadCase !== 'invalid') {
    const text = samples[globalThis.__textUploadCase]();
    await anywhere.clipboard.writeText(text);
    if ((await anywhere.clipboard.readText()) !== text)
      throw new Error('Clipboard round trip changed text');
  } else
    for (const [text, code] of [
      ['\ud800', 'INVALID_REQUEST'],
      ['a'.repeat(1048577), 'TOO_LARGE'],
    ]) {
      let rejected = false;
      try {
        await anywhere.clipboard.writeText(text);
      } catch (error) {
        if (error.code !== code) throw error;
        rejected = true;
      }
      if (!rejected) throw new Error('Invalid text was accepted');
    }
  // The existing harness checks this final value using a synthetic clipboard.
  await anywhere.clipboard.writeText(JSON.stringify(input.value));
}
