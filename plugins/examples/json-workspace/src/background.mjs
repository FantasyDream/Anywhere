import {anywhere} from '@anywhere/sdk';
import {resolveJsonContexts} from './contexts.mjs';
import {copyJson} from './background-actions.mjs';

export async function resolve() {
  return resolveJsonContexts(await anywhere.context.get());
}
export const copyFormatted = () => copyJson(anywhere, 'copy-format');
export const copyMinified = () => copyJson(anywhere, 'copy-minify');
export const copyEscaped = () => copyJson(anywhere, 'copy-escaped');
