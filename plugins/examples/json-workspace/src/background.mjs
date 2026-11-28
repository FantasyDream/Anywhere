import {anywhere, defineResolver} from '@anywhere/sdk';
import {resolveJsonContexts} from './contexts.mjs';
import {copyJson} from './background-actions.mjs';

export const resolve = defineResolver(async ({context}) =>
  resolveJsonContexts(await context.get()),
);
export const copyFormatted = () => copyJson(anywhere, 'copy-format');
export const copyMinified = () => copyJson(anywhere, 'copy-minify');
export const copyEscaped = () => copyJson(anywhere, 'copy-escaped');
