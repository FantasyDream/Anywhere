import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { createHost } from '../dist/index.js';

// Run the actual document-start Host transport, not a second implementation.
const source = readFileSync(new URL('../../../apps/flutter/lib/web_plugins/web_bootstrap.dart', import.meta.url), 'utf8');
const bootstrap = source.match(/const webBootstrap = r'''([\s\S]*?)''';/)[1];

function page() {
  const events = new EventTarget();
  const context = vm.createContext({ TextEncoder, TextDecoder, ArrayBuffer, Uint8Array,
    addEventListener: events.addEventListener.bind(events),
    dispatchEvent: events.dispatchEvent.bind(events), document: {},
  });
  context.window = context;
  context.top = context;
  context.anywhereNative = { postMessage() {} };
  vm.runInContext(bootstrap, context);
  return {
    context,
    transport: context.__anywhereTransport,
    reply(value) {
      // The pinned macOS adapter emits binary replies as a numeric typed array.
      context.replyBytes = [...new TextEncoder().encode(JSON.stringify(value))];
      vm.runInContext('anywhereNative.onmessage({data:new Uint8Array(replyBytes).buffer})', context);
    },
  };
}

test('canceled requests and native send failures do not strand transport capacity', async t => {
  const current = page();
  const client = createHost(current.transport);
  t.after(() => client.dispose());
  // No native replies: abort must release transport accounting as well as SDK promises.
  for (let i = 0; i < 40; i++) {
    const controller = new AbortController();
    const request = client.storage.get('pref', { signal: controller.signal });
    controller.abort();
    await assert.rejects(request, { code: 'CANCELLED' });
  }
  current.context.anywhereNative.postMessage = () => { throw new Error('native closed'); };
  for (let i = 0; i < 40; i++) {
    await assert.rejects(client.ready(), { code: 'HOST_FAILURE' });
  }
  current.context.anywhereNative.postMessage = frame => {
    const request = JSON.parse(frame);
    current.reply({ requestId: request.requestId, result: { value: null } });
  };
  assert.equal(await client.storage.get('recovered'), null);
});

test('native binary invocation reaches the SDK with quotes, escapes and Unicode intact', async t => {
  const current = page();
  const text = ` {"path":"C:\\\\work\\\\project","quote":"\\\"","name":"中文🚀","line":"\\n"}\n`;
  const invocation = { plugin_id: 'dev.example.json', command_id: 'open', entrypoint: 'workspace',
    input: { kind: 'text', source: 'user_input', text } };
  current.context.anywhereNative.postMessage = frame => {
    const call = JSON.parse(frame);
    current.reply({ requestId: call.requestId, result: { value: invocation } });
  };
  const client = createHost(current.transport);
  t.after(() => client.dispose());
  assert.deepEqual(await client.invocation.get({ signal: AbortSignal.timeout(1000) }), invocation);
});

test('native binary results deliver escaped storage values and errors', async t => {
  const current = page();
  const stored = { text: "quote' \\\" \\n \u2028 \u2029 😀", indent: 4 };
  current.context.anywhereNative.postMessage = frame => {
    const call = JSON.parse(frame);
    current.reply(call.method === 'storage.get'
      ? { requestId: call.requestId, result: { value: stored } }
      : { requestId: call.requestId, error: { code: 'PERMISSION_DENIED' } });
  };
  const client = createHost(current.transport);
  t.after(() => client.dispose());
  assert.deepEqual(await client.storage.get('prefs', { signal: AbortSignal.timeout(1000) }), stored);
  await assert.rejects(client.context.getCurrentApp({ signal: AbortSignal.timeout(1000) }), { code: 'PERMISSION_DENIED' });
});

test('document transport rejects bad frames and preserves hidden/resume revocation', () => {
  const current = page();
  const received = [];
  current.transport.subscribe(frame => received.push(JSON.parse(frame)));
  vm.runInContext('anywhereNative.onmessage({data:new Uint8Array([255]).buffer})', current.context);
  vm.runInContext('anywhereNative.onmessage({data:new Uint8Array(65537).buffer})', current.context);
  current.context.anywhereNative.onmessage({ data: '{"requestId":"unexpected"}' });
  assert.equal(received.length, 0);
  const request = { requestId: 'pending', method: 'invocation.get' };
  current.transport.send(JSON.stringify(request));
  current.context.dispatchEvent(new CustomEvent('anywhere:visibility', { detail: false }));
  assert.deepEqual(received, [{ requestId: 'pending', error: { code: 'INACTIVE' } }]);
  assert.throws(() => current.transport.send(JSON.stringify(request)), /INACTIVE/);
  current.context.dispatchEvent(new CustomEvent('anywhere:visibility', { detail: true }));
  current.transport.send(JSON.stringify(request));
  current.reply({ requestId: 'pending', result: { value: 'resumed' } });
  assert.equal(received.at(-1).result.value, 'resumed');
});
