/** Selected at build time by the anywhere-background export condition.
 * No browser globals, Flutter transport, runtime imports or style code are needed.
 */
import {HostError} from './errors.js';
import type {Anywhere} from './api.js';
import type {Calls, PluginSnapshot, WebInput} from './contract.js';
export type {Anywhere, Unsubscribe} from './api.js';
export type {WebInput as Input, JsonValue} from './contract.js';
export {HostError} from './errors.js';

declare const __anywhereInvoke: (method: string, params: string) => string;
async function invoke<T>(
  method: keyof Calls,
  params: unknown = {},
): Promise<T> {
  const reply = JSON.parse(__anywhereInvoke(method, JSON.stringify(params)));
  if (reply.error) throw new HostError(reply.error.code);
  return reply.value as T;
}
let snapshot: Promise<PluginSnapshot> | undefined;
const contexts = new Set<(input: WebInput) => void>();
const inputs = new Set<(text: string) => void>();
const state = () => (snapshot ??= invoke<PluginSnapshot>('state.get'));
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
    set: async text => {
      await invoke('input.set', {text});
      snapshot = undefined;
      const next = await state();
      for (const listener of contexts) listener(next.context);
      if (next.input !== null)
        for (const listener of inputs) listener(next.input);
    },
    onChange: callback =>
      subscribe(inputs, callback, () => anywhere.input.get()),
  },
  clipboard: {
    readText: () => invoke('clipboard.readText'),
    writeText: async text => {
      const size = (character: string) => {
        const code = character.codePointAt(0)!;
        if (code >= 0xd800 && code <= 0xdfff)
          throw new HostError('INVALID_REQUEST');
        return code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
      };
      let bytes = 0;
      for (const character of text) bytes += size(character);
      if (bytes > 1024 * 1024) throw new HostError('TOO_LARGE');
      if (bytes < 8192) return invoke('clipboard.writeText', {text});
      const {ticket} = await invoke<{ticket: string}>('text.begin', {bytes});
      try {
        let chunk = '',
          chunkBytes = 0,
          sequence = 0;
        for (const character of text) {
          const bytes = size(character);
          if (chunkBytes + bytes > 4096) {
            await invoke('text.chunk', {
              ticket,
              sequence: sequence++,
              text: chunk,
            });
            chunk = '';
            chunkBytes = 0;
          }
          chunk += character;
          chunkBytes += bytes;
        }
        if (chunk) await invoke('text.chunk', {ticket, sequence, text: chunk});
        await invoke('text.commit', {ticket});
      } catch (error) {
        await invoke('text.cancel', {ticket}).catch(() => {});
        throw error;
      }
    },
  },
  app: {
    getSource: () => invoke('app.getSource'),
    getFrontmost: () => invoke('app.getFrontmost'),
  },
};
