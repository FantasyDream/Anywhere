import test from 'node:test';
import assert from 'node:assert/strict';
import {createHost} from '../dist/index.js';

const ticket = 'a'.repeat(64);
const chunkBytes = 4096;
const textBytes = 1048576;
let connectionId = 0;

async function connect(runtime, handler) {
  const calls = [];
  function dispatch(method, params) {
    calls.push({method, params});
    return handler(method, params);
  }
  if (runtime === 'background') {
    globalThis.__anywhereInvoke = (method, raw) =>
      JSON.stringify(dispatch(method, JSON.parse(raw)));
    const {anywhere} = await import(
      `../dist/background.js?text-upload-${++connectionId}`
    );
    return {
      clipboard: anywhere.clipboard,
      calls,
      close: () => delete globalThis.__anywhereInvoke,
    };
  }
  let receive;
  const client = createHost({
    send(frame) {
      assert.ok(Buffer.byteLength(frame) <= 65536);
      const call = JSON.parse(frame);
      receive(
        JSON.stringify({
          requestId: call.requestId,
          ...dispatch(call.method, call.params),
        }),
      );
    },
    subscribe(callback) {
      receive = callback;
      return () => {};
    },
  });
  return {clipboard: client.clipboard, calls, close: () => client.dispose()};
}

function result(value = null) {
  // Browser envelopes have result.value; background envelopes have value.
  return {value, result: {value}};
}

for (const runtime of ['browser', 'background']) {
  for (const [name, text] of [
    [
      'Unicode at chunk boundaries',
      'a'.repeat(4095) + '🚀\ufeff中é\u0000'.repeat(10000),
    ],
    ['a full MiB', '中🚀'.repeat(149796) + 'test'],
  ]) {
    test(`${runtime}: ${name} reaches commit without splitting a character`, async t => {
      let uploaded = '',
        sequence = 0,
        declared;
      const connection = await connect(runtime, (method, params) => {
        if (method === 'text.begin') {
          declared = params.bytes;
          return result({ticket});
        }
        if (method === 'text.chunk') {
          assert.equal(params.ticket, ticket);
          assert.equal(params.sequence, sequence++);
          assert.ok(Buffer.byteLength(params.text) <= chunkBytes);
          assert.ok(params.text.length > 0 && params.text.isWellFormed());
          uploaded += params.text;
        } else assert.equal(method, 'text.commit');
        return result();
      });
      t.after(connection.close);
      await connection.clipboard.writeText(text);
      assert.equal(uploaded, text);
      assert.equal(declared, Buffer.byteLength(text));
      assert.equal(connection.calls.at(-1).method, 'text.commit');
      assert.equal(
        connection.calls.filter(c => c.method === 'text.commit').length,
        1,
      );
    });
  }

  test(`${runtime}: direct-write thresholds preserve the transport framing policy`, async () => {
    const samples = [
      ['', true],
      ['\ufeffsmall🚀', true],
      ['a'.repeat(8191), true],
      ['a'.repeat(8192), runtime === 'browser'],
      ['\u0001'.repeat(20000), false],
      ['a'.repeat(65269), runtime === 'browser'],
      ['a'.repeat(65270), false],
    ];
    for (const [text, direct] of samples) {
      const connection = await connect(runtime, method =>
        method === 'text.begin' ? result({ticket}) : result(),
      );
      try {
        await connection.clipboard.writeText(text);
        assert.equal(
          connection.calls[0].method,
          direct ? 'clipboard.writeText' : 'text.begin',
          `unexpected route for ${text.length} characters`,
        );
        assert.equal(
          connection.calls.at(-1).method,
          direct ? 'clipboard.writeText' : 'text.commit',
        );
      } finally {
        connection.close();
      }
    }
  });

  test(`${runtime}: invalid Unicode, size limits and pre-cancellation send no request`, async t => {
    const connection = await connect(runtime, () =>
      assert.fail('unexpected request'),
    );
    t.after(connection.close);
    for (const text of ['\ud800', '\udc00', 'prefix\ud800tail'])
      await assert.rejects(connection.clipboard.writeText(text), {
        code: 'INVALID_REQUEST',
      });
    await assert.rejects(
      connection.clipboard.writeText('a'.repeat(textBytes + 1)),
      {code: 'TOO_LARGE'},
    );
    // Preserve each adapter's validation precedence for compound-invalid input.
    await assert.rejects(
      connection.clipboard.writeText('a'.repeat(textBytes) + '\ud800'),
      {
        code: runtime === 'browser' ? 'TOO_LARGE' : 'INVALID_REQUEST',
      },
    );
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      connection.clipboard.writeText('cancelled', {signal: controller.signal}),
      {code: 'CANCELLED'},
    );
    assert.equal(connection.calls.length, 0);
  });

  for (const phase of ['text.begin', 'text.chunk', 'text.commit']) {
    test(`${runtime}: ${phase} failure preserves the error and never retries the write`, async t => {
      const connection = await connect(runtime, (method, params) => {
        if (
          method === phase &&
          (phase !== 'text.chunk' || params.sequence === 1)
        )
          return {error: {code: 'PERMISSION_DENIED'}};
        if (method === 'text.cancel') return {error: {code: 'HOST_FAILURE'}};
        return method === 'text.begin' ? result({ticket}) : result();
      });
      t.after(connection.close);
      await assert.rejects(connection.clipboard.writeText('x'.repeat(70000)), {
        code: 'PERMISSION_DENIED',
      });
      assert.equal(
        connection.calls.filter(c => c.method === 'text.begin').length,
        1,
      );
      assert.equal(
        connection.calls.filter(c => c.method === 'text.cancel').length,
        phase === 'text.begin' ? 0 : 1,
      );
      assert.equal(
        connection.calls.filter(c => c.method === 'text.commit').length,
        phase === 'text.commit' ? 1 : 0,
      );
      if (phase === 'text.chunk')
        assert.deepEqual(
          connection.calls
            .filter(c => c.method === phase)
            .map(c => c.params.sequence),
          [0, 1],
        );
    });
  }

  test(`${runtime}: cancellation between chunks still cancels the ticket without its signal`, async t => {
    const controller = new AbortController();
    const connection = await connect(runtime, (method, params) => {
      if (method === 'text.chunk') {
        assert.equal(params.sequence, 0);
        queueMicrotask(() => controller.abort());
      }
      return method === 'text.begin' ? result({ticket}) : result();
    });
    t.after(connection.close);
    await assert.rejects(
      connection.clipboard.writeText('x'.repeat(70000), {
        signal: controller.signal,
      }),
      {code: 'CANCELLED'},
    );
    assert.deepEqual(
      connection.calls.map(c => c.method),
      ['text.begin', 'text.chunk', 'text.cancel'],
    );
    assert.equal(connection.calls.at(-1).params.ticket, ticket);
  });
}
