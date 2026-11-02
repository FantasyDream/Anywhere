import test from 'node:test';
import assert from 'node:assert/strict';
import {createHost, host} from '../dist/index.js';
const ticket = 'a'.repeat(64);

function mock(handler = () => {}) {
  const calls = [],
    listeners = new Set();
  const transport = {
    send(raw) {
      assert.ok(Buffer.byteLength(raw) <= 65536);
      const call = JSON.parse(raw);
      calls.push(call);
      handler(call, respond, raw);
    },
    subscribe(receive) {
      listeners.add(receive);
      return () => listeners.delete(receive);
    },
  };
  function respond(call, value) {
    for (const receive of listeners)
      receive(JSON.stringify({requestId: call.requestId, result: {value}}));
  }
  return {
    calls,
    transport,
    respond,
    receive(raw) {
      for (const listener of listeners) listener(raw);
    },
  };
}

test('explicit APIs frame requests and do not expose generic host invocation', async () => {
  const fake = mock((call, reply) =>
    reply(call, {id: 'captured.app', name: 'Captured'}),
  );
  const client = createHost(fake.transport);
  assert.equal(client.invoke, undefined);
  assert.deepEqual(await client.context.getCurrentApp(), {
    id: 'captured.app',
    name: 'Captured',
  });
  assert.equal(fake.calls[0].method, 'context.getCurrentApp');
  assert.deepEqual(Object.keys(fake.calls[0]).sort(), [
    'method',
    'params',
    'protocol',
    'requestId',
  ]);
  client.dispose();
  await assert.rejects(client.ready(), {code: 'INACTIVE'});
  assert.throws(() => host.invocation.get(), {code: 'HOST_UNAVAILABLE'});
});

test('queue is bounded and disposing rejects every pending or queued promise', async () => {
  const fake = mock();
  const client = createHost(fake.transport);
  const pending = Array.from({length: 24}, () => client.storage.get('prefs'));
  assert.equal(fake.calls.length, 8);
  await assert.rejects(client.storage.get('overflow'), {code: 'BUSY'});
  client.dispose();
  const results = await Promise.allSettled(pending);
  assert.ok(
    results.every(v => v.status === 'rejected' && v.reason.code === 'INACTIVE'),
  );
  assert.equal(fake.calls.filter(c => c.method === 'request.cancel').length, 8);
  assert.deepEqual(
    fake.calls
      .filter(c => c.method === 'request.cancel')
      .map(c => c.params.request_id),
    fake.calls.slice(0, 8).map(c => c.requestId),
  );
});

test('aborting a sent clipboard request cancels it without retrying the side effect', async () => {
  const fake = mock();
  const client = createHost(fake.transport);
  const controller = new AbortController();
  const pending = client.clipboard.writeText('copy once', {
    signal: controller.signal,
  });
  controller.abort();
  await assert.rejects(pending, {code: 'CANCELLED'});
  assert.deepEqual(
    fake.calls.map(c => c.method),
    ['clipboard.writeText', 'request.cancel'],
  );
  fake.respond(fake.calls[0], null); // Late completion is ignored.
  client.dispose();
});

for (const [name, text] of [
  ['large Unicode writes preserve BOM and emoji', '中🚀\ufeff\n'.repeat(20000)],
  [
    'escaped text below the byte limit still uses chunks',
    '\u0001'.repeat(20000),
  ],
]) {
  test(name, async () => {
    let result = '',
      sequence = 0,
      declared = 0;
    const fake = mock((call, reply, raw) => {
      if (call.method === 'text.begin') {
        declared = call.params.bytes;
        reply(call, {ticket});
      } else if (call.method === 'text.chunk') {
        assert.ok(Buffer.byteLength(raw) <= 32768);
        assert.ok(Buffer.byteLength(call.params.text) <= 4096);
        assert.equal(call.params.sequence, sequence++);
        result += call.params.text;
        reply(call, null);
      } else if (call.method === 'text.commit') reply(call, null);
      else assert.fail(`unexpected ${call.method}`);
    });
    const client = createHost(fake.transport);
    await client.clipboard.writeText(text);
    assert.equal(result, text);
    assert.equal(declared, Buffer.byteLength(text));
    client.dispose();
  });
}

test('oversized text fails locally and malformed transfer replies are rejected', async () => {
  const fake = mock();
  const client = createHost(fake.transport);
  await assert.rejects(client.clipboard.writeText('中'.repeat(400000)), {
    code: 'TOO_LARGE',
  });
  assert.equal(fake.calls.length, 0);
  const pending = client.invocation.get();
  fake.receive(
    JSON.stringify({
      requestId: fake.calls[0].requestId,
      result: {transfer: {ticket, bytes: 3000000}},
    }),
  );
  await assert.rejects(pending, {code: 'INVALID_RESPONSE'});
  client.dispose();
});

test('download transfer validates sequence and cancels malformed streams', async () => {
  const fake = mock((call, reply) => {
    if (call.method === 'text.read')
      reply(call, {sequence: 9, text: 'bad', done: true});
    else if (call.method === 'text.cancel') reply(call, null);
  });
  const client = createHost(fake.transport);
  const pending = client.invocation.get();
  fake.receive(
    JSON.stringify({
      requestId: fake.calls[0].requestId,
      result: {transfer: {ticket, bytes: 3}},
    }),
  );
  await assert.rejects(pending, {code: 'INVALID_RESPONSE'});
  assert.equal(fake.calls.at(-1).method, 'text.cancel');
  client.dispose();
});

test('state snapshot and pushed updates share one cache and reject old revisions', async () => {
  let initial;
  const fake = mock((call, reply) => {
    if (call.method === 'state.subscribe')
      initial = () =>
        reply(call, {
          version: 1,
          context: {kind: 'text', source: 'selected_text', text: 'old'},
          input: 'old',
        });
  });
  const client = createHost(fake.transport);
  const contexts = [],
    inputs = [];
  const stopContext = client.context.onChange(value => contexts.push(value));
  const stopInput = client.input.onChange(value => inputs.push(value));
  const query = client.input.get();
  await new Promise(setImmediate);
  assert.equal(fake.calls.length, 1);
  fake.receive(
    JSON.stringify({
      event: 'state.changed',
      version: 2,
      state: {
        version: 2,
        context: {kind: 'text', source: 'user_input', text: 'new'},
        input: 'new',
      },
    }),
  );
  initial();
  assert.equal(await query, 'new');
  assert.equal((await client.context.get()).text, 'new');
  assert.deepEqual(inputs, ['new']);
  stopContext();
  stopInput();
  fake.receive(
    JSON.stringify({
      event: 'state.changed',
      version: 3,
      state: {
        version: 3,
        context: {kind: 'empty', reason: 'no_object'},
        input: 'next',
      },
    }),
  );
  assert.equal(contexts.length, 1);
  assert.equal(inputs.length, 1);
  client.dispose();
  await assert.rejects(client.input.set('late'), {code: 'INACTIVE'});
});

test('missing input grants and new app methods remain distinct typed calls', async () => {
  const fake = mock((call, reply) =>
    reply(
      call,
      call.method === 'state.subscribe'
        ? {
            version: 1,
            context: {kind: 'empty', reason: 'permission_denied'},
            input: null,
          }
        : {id: call.method, name: call.method},
    ),
  );
  const client = createHost(fake.transport);
  await assert.rejects(client.input.get(), {code: 'PERMISSION_DENIED'});
  assert.equal((await client.app.getSource()).id, 'app.getSource');
  assert.equal((await client.app.getFrontmost()).id, 'app.getFrontmost');
  client.dispose();
});

test('a successful input write invalidates cached reads before its pushed event', async () => {
  let state = {
    version: 1,
    context: {kind: 'empty', reason: 'no_object'},
    input: 'old',
  };
  const fake = mock((call, reply) => {
    if (call.method === 'state.subscribe' || call.method === 'state.get')
      reply(call, state);
    if (call.method === 'input.set') {
      state = {...state, version: 2, input: call.params.text};
      reply(call, null);
    }
  });
  const client = createHost(fake.transport);
  try {
    assert.equal(await client.input.get(), 'old');
    await client.input.set('new');
    assert.equal(await client.input.get(), 'new');
    assert.equal(fake.calls.filter(c => c.method === 'input.set').length, 1);
  } finally {
    client.dispose();
  }
});

test('large-state notifications share a transfer and catch up to the newest version', async () => {
  const state = (version, input) => ({
    version,
    context: {kind: 'text', source: 'user_input', text: 'x'.repeat(60000)},
    input,
  });
  const payload = JSON.stringify(state(2, 'two'));
  let reads = 0,
    held;
  const fake = mock((call, reply) => {
    if (call.method === 'state.subscribe')
      reply(call, {
        version: 1,
        context: {kind: 'empty', reason: 'no_object'},
        input: 'one',
      });
    if (call.method === 'state.get') {
      reads++;
      if (reads === 1)
        fake.receive(
          JSON.stringify({
            requestId: call.requestId,
            result: {transfer: {ticket, bytes: Buffer.byteLength(payload)}},
          }),
        );
      else
        reply(call, {
          version: 3,
          context: {kind: 'empty', reason: 'no_object'},
          input: 'three',
        });
    }
    if (call.method === 'text.read') {
      const deliver = () => {
        const start = call.params.sequence * 4096;
        const end = Math.min(start + 4096, payload.length);
        reply(call, {
          text: payload.slice(start, end),
          sequence: call.params.sequence,
          done: end === payload.length,
        });
      };
      if (call.params.sequence === 0) held = deliver;
      else deliver();
    }
  });
  const client = createHost(fake.transport);
  try {
    await client.input.get();
    fake.receive(JSON.stringify({event: 'state.changed', version: 2}));
    await new Promise(setImmediate);
    assert.equal(typeof held, 'function');
    fake.receive(JSON.stringify({event: 'state.changed', version: 3}));
    await new Promise(setImmediate);
    assert.equal(reads, 1, 'the active transfer owns state admission');
    held();
    assert.equal(await client.input.get(), 'three');
    assert.equal(reads, 2);
  } finally {
    client.dispose();
  }
});

test('a busy state read retries within its lifetime and disposal cancels retry work', async () => {
  let busy = true;
  const fake = mock((call, reply) => {
    if (call.method !== 'state.subscribe') return;
    if (busy)
      fake.receive(
        JSON.stringify({requestId: call.requestId, error: {code: 'BUSY'}}),
      );
    else
      reply(call, {
        version: 1,
        context: {kind: 'empty', reason: 'no_object'},
        input: 'ready',
      });
  });
  const client = createHost(fake.transport);
  const reading = client.input.get();
  await new Promise(setImmediate);
  assert.equal(fake.calls.length, 1);
  busy = false;
  assert.equal(await reading, 'ready');
  assert.equal(fake.calls.length, 2);
  client.dispose();

  const blocked = mock(call =>
    blocked.receive(
      JSON.stringify({requestId: call.requestId, error: {code: 'BUSY'}}),
    ),
  );
  const cancelled = createHost(blocked.transport);
  const pending = cancelled.input.get();
  await new Promise(setImmediate);
  cancelled.dispose();
  const count = blocked.calls.length;
  await assert.rejects(pending, {code: 'INACTIVE'});
  await new Promise(resolve => setTimeout(resolve, 80));
  assert.equal(blocked.calls.length, count);
});

test('a write fences an already running state read without retrying the write', async () => {
  let held;
  const fake = mock((call, reply) => {
    if (call.method === 'state.subscribe')
      held = () =>
        reply(call, {
          version: 1,
          context: {kind: 'empty', reason: 'no_object'},
          input: 'old',
        });
    if (call.method === 'input.set') reply(call, null);
    if (call.method === 'state.get')
      reply(call, {
        version: 2,
        context: {kind: 'empty', reason: 'no_object'},
        input: 'new',
      });
  });
  const client = createHost(fake.transport);
  try {
    const reading = client.input.get();
    await new Promise(setImmediate);
    await client.input.set('new');
    held();
    assert.equal(await reading, 'new');
    assert.equal(fake.calls.filter(c => c.method === 'input.set').length, 1);
    assert.equal(fake.calls.filter(c => c.method === 'state.get').length, 1);
  } finally {
    client.dispose();
  }
});
