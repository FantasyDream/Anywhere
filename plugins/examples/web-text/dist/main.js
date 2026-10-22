import {definePlugin} from './sdk/index.js';
const text = document.querySelector('#text');
const count = document.querySelector('#count');
text.addEventListener('input', () => {
  const length = Array.from(text.value).length;
  count.value = `${length} character${length === 1 ? '' : 's'}`;
});

definePlugin();
