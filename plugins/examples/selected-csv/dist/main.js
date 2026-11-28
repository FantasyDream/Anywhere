import {definePlugin} from './sdk/index.js';

const name = document.querySelector('#name');
const status = document.querySelector('#status');
const content = document.querySelector('#content');
const read = document.querySelector('#read');
const preview = document.querySelector('#preview');
let selected;

const plugin = definePlugin({
  onLaunch({input}) {
    if (input.kind !== 'resources' || input.items.length !== 1) return;
    const [item] = input.items;
    if (!item.name.toLowerCase().endsWith('.csv')) return;
    selected = item;
    name.textContent = item.name;
    read.disabled = !item.readable;
    preview.disabled = !item.previewable;
    if (item.readable || item.previewable) status.textContent = '';
  },
});

read.addEventListener('click', async () => {
  if (!selected?.readable) return;
  read.disabled = true;
  try {
    const metadata = await plugin.anywhere.resources.stat(selected.handle);
    content.textContent = await plugin.anywhere.resources.readText(
      selected.handle,
    );
    status.textContent =
      metadata.bytes === undefined ? '' : `${metadata.bytes} bytes`;
  } catch (error) {
    status.textContent = `Could not read the selected file (${error.code ?? 'TOOL_ERROR'}).`;
  } finally {
    read.disabled = false;
  }
});

preview.addEventListener('click', async () => {
  if (!selected?.previewable) return;
  preview.disabled = true;
  try {
    await plugin.anywhere.preview.open(selected.handle);
  } catch (error) {
    status.textContent = `Could not preview the selected file (${error.code ?? 'TOOL_ERROR'}).`;
  } finally {
    preview.disabled = false;
  }
});
