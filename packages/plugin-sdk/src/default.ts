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
    set: text => current().input.set(text),
    onChange: callback => current().input.onChange(callback),
  },
  clipboard: {
    readText: () => current().clipboard.readText(),
    writeText: text => current().clipboard.writeText(text),
  },
  app: {
    getSource: () => current().app.getSource(),
    getFrontmost: () => current().app.getFrontmost(),
  },
};
