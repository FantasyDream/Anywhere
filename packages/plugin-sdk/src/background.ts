/** Selected at build time by the anywhere-background export condition.
 * No browser globals, Flutter transport, runtime imports or style code are needed.
 */
import {HostError} from './errors.js';
import {inspectText, uploadText} from './text_upload.js';
import type {Anywhere, CallOptions} from './api.js';
import {limits} from './contract.js';
import type {Calls, PluginSnapshot, WebInput} from './contract.js';
export type {Anywhere, Unsubscribe} from './api.js';
export type {WebInput as Input, JsonValue} from './contract.js';
export {HostError} from './errors.js';

declare const __anywhereInvoke: (method: string, params: string) => string;
async function invoke<T>(
  method: keyof Calls,
  params: unknown = {},
  options?: CallOptions,
): Promise<T> {
  if (options?.signal?.aborted) throw new HostError('CANCELLED');
  const reply = JSON.parse(__anywhereInvoke(method, JSON.stringify(params)));
  if (reply.error) throw new HostError(reply.error.code);
  return reply.value as T;
}
let snapshot: Promise<PluginSnapshot> | undefined;
const contexts = new Set<(input: WebInput) => void>();
const inputs = new Set<(text: string) => void>();
function state() {
  if (snapshot) return snapshot;
  const reading = invoke<PluginSnapshot>('state.get');
  snapshot = reading;
  void reading.catch(() => {
    if (snapshot === reading) snapshot = undefined;
  });
  return reading;
}
function notify<T>(listeners: Set<(value: T) => void>, value: T) {
  for (const listener of listeners) {
    try {
      listener(value);
    } catch {
      /* A subscriber cannot fail the completed write or interrupt other listeners. */
    }
  }
}
function subscribe<T>(
  listeners: Set<(value: T) => void>,
  callback: (value: T) => void,
  value: () => Promise<T>,
) {
  listeners.add(callback);
  void value()
    .then(initial => {
      if (listeners.has(callback)) callback(initial);
    })
    .catch(() => {});
  return () => {
    listeners.delete(callback);
  };
}
export const anywhere: Anywhere = {
  context: {
    get: async () => (await state()).context,
    onChange: callback =>
      subscribe(contexts, callback, () => anywhere.context.get()),
  },
  input: {
    get: async () => {
      const input = (await state()).input;
      if (input === null) throw new HostError('PERMISSION_DENIED');
      return input;
    },
    set: async (text, options) => {
      await invoke('input.set', {text}, options);
      snapshot = undefined;
      if (!contexts.size && !inputs.size) return;
      try {
        const next = await state();
        notify(contexts, next.context);
        if (next.input !== null) notify(inputs, next.input);
      } catch {
        // Readback cannot fail an acknowledged write. A later getter can retry.
      }
    },
    onChange: callback =>
      subscribe(inputs, callback, () => anywhere.input.get()),
  },
  clipboard: {
    readText: options => invoke('clipboard.readText', {}, options),
    writeText: async (text, options) => {
      if (options?.signal?.aborted) throw new HostError('CANCELLED');
      const {bytes, wellFormed} = inspectText(text);
      if (!wellFormed) throw new HostError('INVALID_REQUEST');
      if (bytes > limits.textBytes) throw new HostError('TOO_LARGE');
      if (bytes < 8192) return invoke('clipboard.writeText', {text}, options);
      return uploadText(text, bytes, {
        begin: bytes => invoke('text.begin', {bytes}, options),
        chunk: params => invoke('text.chunk', params, options),
        commit: ticket => invoke('text.commit', {ticket}, options),
        cancel: ticket => invoke('text.cancel', {ticket}),
      });
    },
  },
  storage: {
    get: (key, options) => invoke('storage.get', {key}, options),
    set: (key, value, options) => invoke('storage.set', {key, value}, options),
    remove: (key, options) => invoke('storage.remove', {key}, options),
  },
  app: {
    getSource: options => invoke('app.getSource', {}, options),
    getFrontmost: options => invoke('app.getFrontmost', {}, options),
  },
};
