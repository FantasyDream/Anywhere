import test from 'node:test';
import assert from 'node:assert/strict';
import {environment} from '../dist/index.js';

test('language metadata falls back safely and subscriptions release listeners', () => {
  const events = new EventTarget();
  globalThis.addEventListener = events.addEventListener.bind(events);
  globalThis.removeEventListener = events.removeEventListener.bind(events);
  delete globalThis.__anywhereLanguage;
  assert.equal(environment.language, 'en');
  globalThis.__anywhereLanguage = 'zh';
  assert.equal(environment.language, 'zh');
  const values = [];
  const stop = environment.onLanguageChange(value => values.push(value));
  events.dispatchEvent(new CustomEvent('anywhere:language', {detail: 'zh'}));
  events.dispatchEvent(
    new CustomEvent('anywhere:language', {detail: 'unsupported'}),
  );
  stop();
  events.dispatchEvent(new CustomEvent('anywhere:language', {detail: 'en'}));
  assert.deepEqual(values, ['zh', 'en']);
  globalThis.__anywhereLanguage = '<script>';
  assert.equal(environment.language, 'en');
  delete globalThis.__anywhereLanguage;
});
