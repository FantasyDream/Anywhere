import test from 'node:test';
import assert from 'node:assert/strict';
import {i18n, resources, t} from '../src/i18n.mjs';
import {prepareJsonAction, prepareJsonLaunch} from '../src/actions.mjs';

test('both embedded languages cover every message and format counts', async () => {
  assert.deepEqual(Object.keys(resources.en.translation).sort(), Object.keys(resources.zh.translation).sort());
  await i18n.changeLanguage('en');
  assert.equal(t('match', {count:1}), '1 match');
  assert.equal(t('match', {count:2}), '2 matches');
  await i18n.changeLanguage('zh');
  assert.equal(t('match', {count:2}), '2 项匹配');
  assert.equal(t('spaces', {count:4}), '4 个空格');
  assert.equal(t('calculate'), '计算');
  assert.equal(t(prepareJsonAction('{}', 'copy-format').message), '已复制格式化 JSON');
  assert.equal(t(prepareJsonLaunch({input:{kind:'text', text:JSON.stringify('{}')}, entrypoint:'workspace'}).message), '已解码转义 JSON');
  await i18n.changeLanguage('unsupported');
  assert.equal(t('calculate'), 'Calculate');
  assert.ok(!t('bytes', {count:1024}).includes('{{'));
});
