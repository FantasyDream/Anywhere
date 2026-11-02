import {definePlugin} from './plugin.js';
import {HostError} from './errors.js';
import type {Anywhere} from './api.js';
let plugin: ReturnType<typeof definePlugin> | undefined;
function current() {
  if (!globalThis.__anywhereTransport) throw new HostError('HOST_UNAVAILABLE');
  return (plugin ??= definePlugin({
    onDispose: () => {
      plugin = undefined;
    },
  })).anywhere;
}
/** Zero-setup facade. Startup, visibility, rebind and disposal belong to the SDK. */
export const anywhere: Anywhere = {
  context: {
    get: () => current().context.get(),
    onChange: callback => current().context.onChange(callback),
  },
  input: {
    get: () => current().input.get(),
    set: (text, options) => current().input.set(text, options),
    onChange: callback => current().input.onChange(callback),
  },
  clipboard: {
    readText: options => current().clipboard.readText(options),
    writeText: (text, options) => current().clipboard.writeText(text, options),
  },
  storage: {
    get: (key, options) => current().storage.get(key, options),
    set: (key, value, options) => current().storage.set(key, value, options),
    remove: (key, options) => current().storage.remove(key, options),
  },
  app: {
    getSource: options => current().app.getSource(options),
    getFrontmost: options => current().app.getFrontmost(options),
  },
};
