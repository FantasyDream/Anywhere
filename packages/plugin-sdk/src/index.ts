// Public entry point; implementation modules depend only on lower layers.
export * from './client.js';
export {lifecycle} from './lifecycle.js';
export {definePlugin} from './plugin.js';
export type {PluginOptions} from './plugin.js';
export {environment} from './environment.js';
export type {Language} from './environment.js';
export type {Anywhere, Unsubscribe} from './api.js';
export {anywhere} from './default.js';
