import gts from 'gts';
import globals from 'globals';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));

export default [
  ...gts,
  {
    files: ['**/*.{js,mjs,ts,jsx}'],
    languageOptions: {globals: globals.browser},
    rules: {'prettier/prettier': ['error', {}, {usePrettierrc: true}]},
  },
  {
    files: ['**/test/**/*.mjs', '**/tools/**/*.mjs', '**/build.mjs'],
    languageOptions: {globals: globals.node},
  },
  {
    files: ['**/*.cjs'],
    languageOptions: {globals: globals.node, sourceType: 'commonjs'},
  },
  {
    files: [
      'plugins/examples/background-context/**/*.ts',
      'packages/plugin-sdk/templates/background/**/*.ts',
    ],
    languageOptions: {
      parserOptions: {
        project: './tools/code-style/tsconfig-plugins.json',
        tsconfigRootDir: root,
      },
    },
  },
  {
    files: ['packages/plugin-sdk/src/**/*.ts'],
    languageOptions: {
      parserOptions: {
        project: './packages/plugin-sdk/tsconfig.json',
        tsconfigRootDir: root,
      },
    },
  },
];
