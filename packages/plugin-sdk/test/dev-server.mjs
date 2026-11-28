import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';

const child = spawn('npm', ['run', 'dev'], {
  cwd: process.argv[2],
  detached: true,
  stdio: ['ignore', 'pipe', 'inherit'],
});
const done = once(child, 'exit');
const controller = new AbortController();
function stop() {
  try {
    process.kill(-child.pid, 'SIGTERM');
  } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
}
const deadline = setTimeout(() => {
  controller.abort();
  stop();
}, 10000);
try {
  let output = '';
  let base;
  for await (const chunk of child.stdout) {
    output += chunk;
    base = output.match(/Anywhere Mock at (http:\/\/127\.0\.0\.1:\d+)/)?.[1];
    if (base) break;
    if (controller.signal.aborted) break;
  }
  assert.ok(base, output);
  const html = await fetch(base, {signal: controller.signal});
  assert.match(await html.text(), /src="\/__mock\.js"/);
  const script = await fetch(new URL('/__mock.js', base), {
    signal: controller.signal,
  });
  assert.match(await script.text(), /createMock/);
} finally {
  clearTimeout(deadline);
  stop();
  await done;
}
