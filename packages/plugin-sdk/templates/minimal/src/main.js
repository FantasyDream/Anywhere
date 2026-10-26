import {definePlugin} from '@anywhere/sdk';

const input = document.querySelector('#input');
const status = document.querySelector('#status');
const copy = document.querySelector('#copy');
let edited = false;
const plugin = definePlugin({
  onLaunch({input: initial}) {
    if (!edited && initial.kind === 'text') input.value = initial.text;
    input.focus();
  },
});

input.addEventListener('input', () => {
  edited = true;
});
document.querySelector('#uppercase').addEventListener('click', () => {
  edited = true;
  input.value = input.value.toUpperCase();
  input.focus();
});
copy.addEventListener('click', async () => {
  copy.disabled = true;
  try {
    await plugin.host.clipboard.writeText(input.value);
    status.textContent = 'Copied';
  } catch {
    status.textContent = 'Copy was not completed. You can try again.';
  } finally {
    copy.disabled = false;
  }
});
