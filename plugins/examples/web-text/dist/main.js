import {definePlugin} from './sdk/index.js';
const text = document.querySelector('#text');
const count = document.querySelector('#count');
text.addEventListener('input', () => {
  const length = Array.from(text.value).length;
  count.value = `${length} character${length === 1 ? '' : 's'}`;
});

const {anywhere} = definePlugin({
  async onLaunch() {
    const input = await anywhere.context.get();
    if (input.kind === 'text') text.value = input.text;
  },
});
anywhere.input.onChange(query => {
  document.title = query ? `Text Pad · ${query}` : 'Text Pad';
});
