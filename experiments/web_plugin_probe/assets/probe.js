'use strict';
const output = document.getElementById('result');
const pending = new Map();
let nextId = 0;
function request(method, text) {
  const requestId = `p${++nextId}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('TIMEOUT')); }, 3000);
    pending.set(requestId, { resolve, reject, timer });
    window.anywhereProbe.postMessage(JSON.stringify({ protocol: 1, requestId, method, params: { text } }));
  });
}
window.anywhereProbe.onmessage = event => {
  const reply = JSON.parse(event.data);
  const item = pending.get(reply.requestId);
  if (!item) return;
  clearTimeout(item.timer);
  pending.delete(reply.requestId);
  reply.error ? item.reject(new Error(reply.error.code)) : item.resolve(reply.result);
};
document.getElementById('send').addEventListener('click', async () => {
  try { output.textContent = JSON.stringify(await request('probe.echo', document.getElementById('input').value)); }
  catch (error) { output.textContent = error.message; }
});
(async () => {
  const checks = {};
  try {
    const echo = await request('probe.echo', '你好 😀 <script>');
    checks.echo = echo.text === '你好 😀 <script>';
    checks.epoch = echo.epoch;
    try { await request('clipboard.readText', ''); checks.denied = false; }
    catch (error) { checks.denied = error.message === 'METHOD_NOT_ALLOWED'; }
    try { await fetch('https://127.0.0.1:1/probe-must-not-connect'); checks.connectBlocked = false; }
    catch (_) { checks.connectBlocked = true; }
    // A rejected fetch is only an observation; proving no network egress requires
    // a controlled endpoint/platform trace, recorded separately in P0 acceptance.
    output.textContent = JSON.stringify(checks, null, 2);
    await request('probe.echo', 'CHECKS:' + JSON.stringify(checks));
  } catch (error) { output.textContent = `Probe failed: ${error.message}`; }
})();
