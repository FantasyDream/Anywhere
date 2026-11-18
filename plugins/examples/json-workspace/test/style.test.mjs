import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {test} from 'node:test';

test('fixed dark action menu keeps readable on-dark semantics in light glass', async () => {
  const css = await readFile(
    new URL('../src/style.css', import.meta.url),
    'utf8',
  );
  const start = css.indexOf('.json-menu-panel {');
  const end = css.indexOf('\n}', start);
  assert.notEqual(start, -1);
  const panel = css.slice(start, end);
  assert.match(panel, /color-scheme: dark;/);
  for (const variable of [
    '--aw-text',
    '--aw-primary',
    '--aw-secondary',
    '--aw-muted',
    '--aw-border',
    '--aw-control-border',
    '--aw-divider',
    '--aw-hover',
    '--aw-selected-surface',
    '--aw-selection-outline',
  ]) {
    assert.match(panel, new RegExp(`${variable}: var\\(--aw-on-dark-`));
  }
  assert.match(
    css,
    /\.json-menu-panel > button[^{]*\{[^}]*color: var\(--aw-on-dark-primary\)/,
  );
  assert.match(
    css,
    /\.json-menu-panel select[^{]*\{[^}]*color: var\(--aw-on-dark-primary\)/,
  );
  assert.match(
    css,
    /\.json-menu-panel \.indent[^{]*\{[^}]*color: var\(--aw-on-dark-secondary\)/,
  );
});
