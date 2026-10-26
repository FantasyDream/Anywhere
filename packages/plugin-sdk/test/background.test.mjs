import test from 'node:test';
import assert from 'node:assert/strict';

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
    return JSON.stringify({value: {id: method, name: method}});
  };
  try {
    const {anywhere} = await import('../dist/background.js');
    assert.equal((await anywhere.context.get()).text, 'hello');
    await assert.rejects(anywhere.input.get(), {code: 'PERMISSION_DENIED'});
    assert.equal((await anywhere.app.getSource()).id, 'app.getSource');
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
      ['state.get', 'app.getSource', 'clipboard.writeText'],
    );
  } finally {
    delete globalThis.__anywhereInvoke;
  }
});
