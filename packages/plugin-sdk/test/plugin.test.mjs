import test from 'node:test';
import assert from 'node:assert/strict';
import {definePlugin, anywhere} from '../dist/index.js';

function environment(handler) {
  const events = new EventTarget(),
    listeners = new Set(),
    calls = [];
  globalThis.addEventListener = events.addEventListener.bind(events);
  globalThis.removeEventListener = events.removeEventListener.bind(events);
  globalThis.__anywhereTransport = {
    subscribe(receive) {
      listeners.add(receive);
      return () => listeners.delete(receive);
    },
    send(raw) {
      const call = JSON.parse(raw);
      calls.push(call);
      handler(call, value => {
        for (const receive of listeners)
          receive(JSON.stringify({requestId: call.requestId, result: {value}}));
      });
    },
  };
  return {
    calls,
    listeners,
    visibility(visible) {
      events.dispatchEvent(
        new CustomEvent('anywhere:visibility', {detail: visible}),
      );
    },
    close() {
      events.dispatchEvent(new Event('pagehide'));
      delete globalThis.__anywhereTransport;
    },
  };
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
const launch = {
  input: {kind: 'text', text: 'original'},
  entrypoint: 'workspace',
};

test('automatic launch once, resume preserves drafts and stable host cancels pending operations', async () => {
  let block = false,
    launched = 0,
    signal;
  const env = environment((call, reply) => {
    if (
      call.method === 'request.cancel' ||
      (block && call.method === 'storage.get')
    )
      return;
    reply(call.method === 'invocation.get' ? launch : null);
  });
  const plugin = definePlugin({
    onLaunch(invocation, context) {
      launched++;
      assert.deepEqual(invocation, launch);
      signal = context.signal;
    },
  });
  try {
    await plugin.ready;
    const {host} = plugin;
    block = true;
    const pending = host.storage.get('draft');
    env.visibility(false);
    await assert.rejects(pending, {code: 'INACTIVE'});
    assert.equal(env.listeners.size, 0);
    assert.equal(
      env.calls.filter(call => call.method === 'request.cancel').length,
      1,
    );
    await assert.rejects(host.clipboard.writeText('hidden'), {
      code: 'INACTIVE',
    });
    env.visibility(true);
    await tick();
    assert.equal(launched, 1);
    assert.equal(
      env.calls.filter(call => call.method === 'lifecycle.ready').length,
      2,
    );
    assert.equal(
      env.calls.filter(call => call.method === 'invocation.get').length,
      1,
    );
    block = false;
    await host.storage.get('draft');
    env.close();
    assert.equal(signal.aborted, true);
    assert.equal(env.listeners.size, 0);
    await assert.rejects(host.storage.get('draft'), {code: 'INACTIVE'});
  } finally {
    plugin.dispose();
    env.close();
  }
});

test('hide during startup ignores old input and consumes invocation after resume', async () => {
  let replyInput,
    launched = 0;
  const env = environment((call, reply) => {
    if (call.method === 'lifecycle.ready') reply(null);
    if (call.method === 'invocation.get') replyInput = reply;
  });
  const plugin = definePlugin({
    onLaunch() {
      launched++;
    },
  });
  try {
    await tick();
    const oldReply = replyInput;
    env.visibility(false);
    env.visibility(true);
    await tick();
    oldReply(launch);
    await tick();
    assert.equal(launched, 0);
    replyInput(launch);
    await plugin.ready;
    assert.equal(launched, 1);
  } finally {
    plugin.dispose();
    env.close();
  }
});

test('failed business launch is reported once and disposal before startup creates no connection', async () => {
  let failures = 0;
  const env = environment((call, reply) => {
    if (call.method !== 'request.cancel')
      reply(call.method === 'invocation.get' ? launch : null);
  });
  const plugin = definePlugin({
    onLaunch() {
      throw new Error('business');
    },
    onError() {
      failures++;
    },
  });
  try {
    await plugin.ready;
    env.visibility(false);
    env.visibility(true);
    await tick();
    assert.equal(failures, 1);
    plugin.dispose();
    const before = env.calls.length;
    const disposed = definePlugin();
    disposed.dispose();
    await disposed.ready;
    await tick();
    assert.equal(env.calls.length, before);
  } finally {
    plugin.dispose();
    env.close();
  }
});

test('zero-setup SDK subscribes once and reconnects without retaining hidden clients', async () => {
  let value = 'first';
  const env = environment((call, reply) =>
    reply(
      call.method === 'state.subscribe'
        ? {
            version: value === 'first' ? 1 : 2,
            context: {kind: 'text', source: 'user_input', text: value},
            input: value,
          }
        : call.method === 'invocation.get'
          ? launch
          : null,
    ),
  );
  try {
    assert.equal((await anywhere.context.get()).text, 'first');
    const received = [];
    const stop = anywhere.context.onChange(input => received.push(input.text));
    await tick();
    assert.deepEqual(received, ['first']);
    env.visibility(false);
    assert.equal(env.listeners.size, 0);
    value = 'resumed';
    env.visibility(true);
    await tick();
    assert.deepEqual(received, ['first', 'resumed']);
    assert.equal(env.listeners.size, 1);
    stop();
  } finally {
    env.close();
  }
  assert.equal(env.listeners.size, 0);
});
