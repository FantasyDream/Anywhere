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

test('large Unicode writes use byte-bounded chunks and preserve BOM and emoji', async () => {
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
  const text = '中🚀\ufeff\n'.repeat(20000);
  await client.clipboard.writeText(text);
  assert.equal(result, text);
  assert.equal(declared, Buffer.byteLength(text));
  client.dispose();
});

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
