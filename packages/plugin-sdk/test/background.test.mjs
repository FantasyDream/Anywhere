import test from 'node:test';
import assert from 'node:assert/strict';

test('background subscriber failures do not reject a completed input write or skip other listeners', async () => {
  let text = 'initial',
    writes = 0;
  globalThis.__anywhereInvoke = (method, raw) => {
    if (method === 'input.set') {
      text = JSON.parse(raw).text;
      writes++;
    }
    return JSON.stringify({
      value:
        method === 'state.get'
          ? {
              version: writes + 1,
              context: {kind: 'text', source: 'user_input', text},
              input: text,
            }
          : null,
    });
  };
  const {anywhere} = await import('../dist/background.js?listener-isolation');
  const contexts = [],
    inputs = [];
  const stops = [
    anywhere.context.onChange(value => {
      if (value.text === 'updated')
        throw new Error('broken context subscriber');
    }),
    anywhere.context.onChange(value => contexts.push(value.text)),
    anywhere.input.onChange(value => {
      if (value === 'updated') throw new Error('broken input subscriber');
    }),
    anywhere.input.onChange(value => inputs.push(value)),
  ];
  try {
    await new Promise(resolve => setTimeout(resolve, 0));
    await anywhere.input.set('updated');
    assert.equal(writes, 1);
    assert.deepEqual(contexts, ['initial', 'updated']);
    assert.deepEqual(inputs, ['initial', 'updated']);
  } finally {
    for (const stop of stops) stop();
    delete globalThis.__anywhereInvoke;
  }
});

test('background adapter uses the same typed API without browser globals', async () => {
  const calls = [];
  globalThis.__anywhereInvoke = (method, params) => {
    calls.push({method, params: JSON.parse(params)});
    if (method === 'state.get')
      return JSON.stringify({
        value: {
          version: 1,
          context: {kind: 'text', source: 'selected_text', text: 'hello'},
          input: null,
        },
      });
    if (method === 'clipboard.writeText')
      return JSON.stringify({error: {code: 'PERMISSION_DENIED'}});
    if (method === 'storage.get') return JSON.stringify({value: 4});
    if (method === 'storage.set' || method === 'storage.remove')
      return JSON.stringify({value: null});
    return JSON.stringify({value: {id: method, name: method}});
  };
  try {
    const {anywhere} = await import('../dist/background.js');
    assert.equal((await anywhere.context.get()).text, 'hello');
    await assert.rejects(anywhere.input.get(), {code: 'PERMISSION_DENIED'});
    assert.equal((await anywhere.app.getSource()).id, 'app.getSource');
    assert.equal(await anywhere.storage.get('indent'), 4);
    await anywhere.storage.set('indent', 2);
    await anywhere.storage.remove('indent');
    await assert.rejects(anywhere.clipboard.writeText('private'), {
      code: 'PERMISSION_DENIED',
    });
    const events = [];
    const stop = anywhere.context.onChange(input => events.push(input));
    stop();
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(events.length, 0);
    assert.deepEqual(
      calls.map(call => call.method),
      [
        'state.get',
        'app.getSource',
        'storage.get',
        'storage.set',
        'storage.remove',
        'clipboard.writeText',
      ],
    );
    assert.deepEqual(calls.find(call => call.method === 'storage.set').params, {
      key: 'indent',
      value: 2,
    });
  } finally {
    delete globalThis.__anywhereInvoke;
  }
});

test('background text transport commits a full MiB and respects pre-cancellation', async () => {
  const {anywhere} = await import('../dist/background.js');
  let bytes = 0,
    committed = false,
    calls = 0;
  globalThis.__anywhereInvoke = (method, raw) => {
    calls++;
    const params = JSON.parse(raw);
    if (method === 'text.begin')
      return JSON.stringify({value: {ticket: 'ticket'}});
    if (method === 'text.chunk') bytes += Buffer.byteLength(params.text);
    if (method === 'text.commit') committed = true;
    return JSON.stringify({value: null});
  };
  try {
    await anywhere.clipboard.writeText('中🚀'.repeat(149796) + 'test');
    assert.equal(bytes, 1048576);
    assert.equal(committed, true);
    assert.ok(calls < 1024);
    const before = calls;
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      anywhere.clipboard.writeText('no', {signal: controller.signal}),
      {code: 'CANCELLED'},
    );
    assert.equal(calls, before);
  } finally {
    delete globalThis.__anywhereInvoke;
  }
});
