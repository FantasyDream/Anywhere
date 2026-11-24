import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {test} from 'node:test';

test('JSON actions use an in-flow command list and shared editor colors', async () => {
  const [css, app] = await Promise.all([
    readFile(
      new URL('../../../../resources/web-controls.css', import.meta.url),
      'utf8',
    ),
    readFile(new URL('../src/main.jsx', import.meta.url), 'utf8'),
  ]);
  assert.match(app, /class="aw-command-list json-menu-panel"/);
  assert.match(
    css,
    /:where\(\.aw-command-list\) \{[^}]+flex-direction: column/,
  );
  assert.match(
    css,
    /:where\(input, textarea\)::placeholder \{ color: var\(--aw-muted\)/,
  );
  assert.match(
    css,
    /\.aw-editor\) \{[^}]+background: var\(--aw-editor-surface\)/,
  );
});
