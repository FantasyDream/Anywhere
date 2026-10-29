/** Selected at build time by the anywhere-background export condition.
 * No browser globals, Flutter transport, runtime imports or style code are needed.
 */
import {HostError} from './errors.js';
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
const state = () => (snapshot ??= invoke<PluginSnapshot>('state.get'));
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
      const next = await state();
      notify(contexts, next.context);
      if (next.input !== null) notify(inputs, next.input);
    },
    onChange: callback =>
      subscribe(inputs, callback, () => anywhere.input.get()),
  },
  clipboard: {
    readText: options => invoke('clipboard.readText', {}, options),
    writeText: async (text, options) => {
      if (options?.signal?.aborted) throw new HostError('CANCELLED');
      const size = (character: string) => {
        const code = character.codePointAt(0)!;
        if (code >= 0xd800 && code <= 0xdfff)
          throw new HostError('INVALID_REQUEST');
        return code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
      };
      let bytes = 0;
      for (const character of text) bytes += size(character);
      if (bytes > limits.textBytes) throw new HostError('TOO_LARGE');
      if (bytes < 8192) return invoke('clipboard.writeText', {text}, options);
      const {ticket} = await invoke<{ticket: string}>(
        'text.begin',
        {bytes},
        options,
      );
      try {
        let chunk = '',
          chunkBytes = 0,
          sequence = 0;
        for (const character of text) {
          const bytes = size(character);
          if (chunkBytes + bytes > limits.chunkBytes) {
            await invoke(
              'text.chunk',
              {
                ticket,
                sequence: sequence++,
                text: chunk,
              },
              options,
            );
            chunk = '';
            chunkBytes = 0;
          }
          chunk += character;
          chunkBytes += bytes;
        }
        if (chunk)
          await invoke('text.chunk', {ticket, sequence, text: chunk}, options);
        await invoke('text.commit', {ticket}, options);
      } catch (error) {
        await invoke('text.cancel', {ticket}).catch(() => {});
        throw error;
      }
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
