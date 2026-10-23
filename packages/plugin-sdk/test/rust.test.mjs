import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {once} from 'node:events';
import {createHost} from '../dist/index.js';

async function connect(t) {
  const child = spawn(process.env.ANYWHERE_WEB_BRIDGE_TEST_BIN, [], {
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const lines = createInterface({input: child.stdout});
  let receive = () => {},
    approved = true,
    ready;
  const started = new Promise((resolve, reject) => {
    ready = resolve;
    child.once('error', reject);
    child.once('exit', code => {
      if (code !== 0) reject(new Error(`Rust harness exited ${code}`));
    });
  });
  let stderr = '';
  child.stderr.on('data', data => {
    stderr += data;
  });
  function send(value) {
    child.stdin.write(JSON.stringify(value) + '\n');
  }
  lines.on('line', line => {
    const value = JSON.parse(line);
    if (value.kind === 'ready') ready();
    else if (value.kind === 'reply') receive(value.frame);
    else if (value.kind === 'confirmation')
      send({kind: 'confirm', ticket: value.ticket, approved});
  });
  t.after(async () => {
    child.stdin.end();
    if (child.exitCode === null) await once(child, 'exit');
    assert.equal(child.exitCode, 0, stderr);
    lines.close();
  });
  await started;
  return {
    transport: {
      send(frame) {
        send({kind: 'call', frame});
      },
      subscribe(callback) {
        receive = callback;
        return () => {
          receive = () => {};
        };
      },
    },
    decline() {
      approved = false;
    },
  };
}
const rust = process.env.ANYWHERE_WEB_BRIDGE_TEST_BIN;
test(
  'SDK talks to the real Rust broker for invocation, KV and confirmed clipboard access',
  {skip: !rust, timeout: 15000},
  async t => {
    const connection = await connect(t);
    const client = createHost(connection.transport);
    t.after(() => client.dispose());
    await client.ready();
    const input = await client.invocation.get();
    assert.equal(input.plugin_id, 'dev.example.conformance');
    assert.equal(input.input.kind, 'empty');
    assert.equal(await client.context.getCurrentApp(), null);
    await client.storage.set('prefs', {indent: 2});
    assert.deepEqual(await client.storage.get('prefs'), {indent: 2});
    await client.storage.remove('prefs');
    assert.equal(await client.storage.get('prefs'), null);
    assert.equal(await client.clipboard.readText(), 'synthetic clipboard');
    await client.clipboard.writeText('small copy');
    assert.equal(await client.clipboard.readText(), 'small copy');
  },
);
test(
  'SDK and Rust round-trip a large Unicode clipboard payload without base64',
  {skip: !rust, timeout: 15000},
  async t => {
    const connection = await connect(t);
    const client = createHost(connection.transport);
    t.after(() => client.dispose());
    const text = '中🚀\ufeff\n'.repeat(40000);
    await client.clipboard.writeText(text);
    assert.equal(await client.clipboard.readText(), text);
  },
);
test(
  'Host declining consent reaches SDK as a non-retried cancellation',
  {skip: !rust, timeout: 10000},
  async t => {
    const connection = await connect(t);
    connection.decline();
    const client = createHost(connection.transport);
    t.after(() => client.dispose());
    await assert.rejects(client.clipboard.readText(), {code: 'USER_CANCELLED'});
  },
);
