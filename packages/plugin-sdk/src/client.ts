import { protocol, limits, errorCodes } from './contract.js';
import type { Calls, JsonValue, WebInvocation as Invocation, WebCurrentApp as CurrentApp } from './contract.js';
export type { JsonValue, WebInput as Input, WebInvocation as Invocation, WebCurrentApp as CurrentApp } from './contract.js';

export interface HostTransport {
  send(frame: string): void | Promise<void>;
  subscribe(receive: (frame: string) => void): () => void;
}
export interface CallOptions { signal?: AbortSignal }
export class HostError extends Error {
  readonly code: string;
  constructor(code: string, message = 'The Host request could not be completed.') {
    super(message); this.name = 'HostError'; this.code = code;
  }
}

type Body = { value: unknown } | { transfer: { ticket: string; bytes: number } };
type Job = { id: string; frame: string; timeout: number; options?: CallOptions;
  resolve: (value: Body) => void; reject: (error: unknown) => void;
  timer?: ReturnType<typeof setTimeout>; abort?: () => void };
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
function object(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }

/** No generic invoke API is exported. The injected transport carries no authority from JS. */
export function createHost(transport: HostTransport) {
  const random = crypto.getRandomValues(new Uint32Array(4));
  const prefix = Array.from(random, n => n.toString(16).padStart(8, '0')).join('');
  let sequence = 0, disposed = false;
  const pending = new Map<string, Job>();
  const queue: Job[] = [];
  const id = () => `${prefix}_${++sequence}`;
  function send(frame: string, failed: (error: unknown) => void) {
    try { Promise.resolve(transport.send(frame)).catch(failed); } catch (error) { failed(error); }
  }
  function cancel(requestId: string) {
    // Best effort cancellation is not a retry of the original side effect.
    send(JSON.stringify({ protocol, requestId: id(), method: 'request.cancel', params: { request_id: requestId } }), () => {});
  }
  function settle(job: Job, error?: unknown, value?: Body) {
    const queued = queue.indexOf(job);
    const active = pending.delete(job.id);
    if (!active && queued < 0) return;
    if (queued >= 0) queue.splice(queued, 1);
    clearTimeout(job.timer);
    if (job.abort) job.options?.signal?.removeEventListener('abort', job.abort);
    if (error) job.reject(error); else job.resolve(value!);
    pump();
  }
  function pump() {
    while (!disposed && pending.size < limits.running && queue.length) {
      const job = queue.shift()!;
      pending.set(job.id, job);
      job.timer = setTimeout(() => {
        settle(job, new HostError('TIMEOUT'));
        cancel(job.id);
      }, job.timeout);
      send(job.frame, () => settle(job, new HostError('HOST_FAILURE')));
    }
  }
  const unsubscribe = transport.subscribe(frame => {
    if (disposed || frame.length > limits.frameBytes || encoder.encode(frame).length > limits.frameBytes) return;
    let message: unknown;
    try { message = JSON.parse(frame); } catch { return; }
    if (!object(message) || typeof message.requestId !== 'string') return;
    const job = pending.get(message.requestId);
    if (!job) return;
    if ('error' in message && !('result' in message) && object(message.error)) {
      const code = message.error.code;
      settle(job, new HostError(typeof code === 'string' && (errorCodes as readonly string[]).includes(code) ? code : 'HOST_FAILURE'));
    } else if ('result' in message && !('error' in message) && object(message.result)) {
      const result = message.result;
      if ('value' in result && !('transfer' in result)) settle(job, undefined, { value: result.value });
      else if ('transfer' in result && !('value' in result) && object(result.transfer)
        && typeof result.transfer.ticket === 'string' && /^[a-f0-9]{64}$/.test(result.transfer.ticket)
        && Number.isSafeInteger(result.transfer.bytes) && (result.transfer.bytes as number) > 0
        && (result.transfer.bytes as number) <= limits.transferBytes) {
        settle(job, undefined, { transfer: result.transfer as { ticket: string; bytes: number } });
      } else settle(job, new HostError('INVALID_RESPONSE'));
    } else settle(job, new HostError('INVALID_RESPONSE'));
  });
  function raw<M extends keyof Calls>(method: M, params: Calls[M], options?: CallOptions, timeout: number = limits.timeoutMs): Promise<Body> {
    if (disposed) return Promise.reject(new HostError('INACTIVE'));
    if (options?.signal?.aborted) return Promise.reject(new HostError('CANCELLED'));
    if (pending.size >= limits.running && queue.length >= limits.queue) return Promise.reject(new HostError('BUSY'));
    const requestId = id();
    let frame: string;
    try { frame = JSON.stringify({ protocol, requestId, method, params }); } catch { return Promise.reject(new HostError('INVALID_REQUEST')); }
    if (encoder.encode(frame).length > limits.frameBytes) return Promise.reject(new HostError('TOO_LARGE'));
    return new Promise((resolve, reject) => {
      const job: Job = { id: requestId, frame, options, timeout, resolve, reject };
      job.abort = () => { const sent = pending.has(job.id); settle(job, new HostError('CANCELLED')); if (sent) cancel(job.id); };
      options?.signal?.addEventListener('abort', job.abort, { once: true });
      queue.push(job); pump();
    });
  }
  async function call<M extends keyof Calls, T = unknown>(method: M, params: Calls[M], options?: CallOptions, timeout?: number): Promise<T> {
    const body = await raw(method, params, options, timeout);
    if ('value' in body) return body.value as T;
    const { ticket, bytes } = body.transfer;
    const chunks: string[] = [];
    let count = 0;
    try {
      for (let sequence = 0; ; sequence++) {
        const part = await raw('text.read', { ticket, sequence }, options);
        if (!('value' in part) || !object(part.value) || typeof part.value.text !== 'string'
          || part.value.sequence !== sequence || typeof part.value.done !== 'boolean') throw new HostError('INVALID_RESPONSE');
        const size = encoder.encode(part.value.text).length;
        if (size > limits.chunkBytes || size === 0 || (count += size) > bytes) throw new HostError('INVALID_RESPONSE');
        chunks.push(part.value.text);
        if (part.value.done) break;
      }
      if (count !== bytes) throw new HostError('INVALID_RESPONSE');
      return JSON.parse(chunks.join('')) as T;
    } catch (error) {
      void raw('text.cancel', { ticket }).catch(() => {});
      throw error;
    }
  }
  async function writeText(text: string, options?: CallOptions): Promise<void> {
    const bytes = encoder.encode(text);
    if (bytes.length > limits.textBytes) throw new HostError('TOO_LARGE');
    if (decoder.decode(bytes) !== text) throw new HostError('INVALID_REQUEST');
    // Reserve framing overhead so escaped text automatically takes the chunk path.
    if (encoder.encode(JSON.stringify({ text })).length + 256 <= limits.frameBytes) {
      return call('clipboard.writeText', { text }, options, limits.confirmTimeoutMs);
    }
    const { ticket } = await call<'text.begin', { ticket: string }>('text.begin', { bytes: bytes.length }, options);
    try {
      for (let offset = 0, sequence = 0; offset < bytes.length; sequence++) {
        let end = Math.min(offset + limits.chunkBytes, bytes.length);
        while (end < bytes.length && (bytes[end]! & 0xc0) === 0x80) end--;
        await call('text.chunk', { ticket, sequence, text: decoder.decode(bytes.subarray(offset, end)) }, options);
        offset = end;
      }
      await call('text.commit', { ticket }, options, limits.confirmTimeoutMs);
    } catch (error) {
      void raw('text.cancel', { ticket }).catch(() => {}); throw error;
    }
  }
  return {
    ready: (options?: CallOptions) => call<'lifecycle.ready', void>('lifecycle.ready', {}, options),
    invocation: { get: (options?: CallOptions) => call<'invocation.get', Invocation>('invocation.get', {}, options) },
    context: { getCurrentApp: (options?: CallOptions) => call<'context.getCurrentApp', CurrentApp | null>('context.getCurrentApp', {}, options) },
    clipboard: { readText: (options?: CallOptions) => call<'clipboard.readText', string | null>('clipboard.readText', {}, options, limits.confirmTimeoutMs), writeText },
    storage: {
      get: <T extends JsonValue = JsonValue>(key: string, options?: CallOptions) => call<'storage.get', T | null>('storage.get', { key }, options),
      set: (key: string, value: JsonValue, options?: CallOptions) => call<'storage.set', void>('storage.set', { key, value }, options),
      remove: (key: string, options?: CallOptions) => call<'storage.remove', void>('storage.remove', { key }, options),
    },
    dispose() {
      if (disposed) return;
      disposed = true; unsubscribe();
      const sent = [...pending.keys()];
      for (const job of [...queue, ...pending.values()]) settle(job, new HostError('INACTIVE'));
      for (const requestId of sent) cancel(requestId);
    },
  };
}

declare global { var __anywhereTransport: HostTransport | undefined; }
let defaultHost: ReturnType<typeof createHost> | undefined;
function injected() {
  if (!defaultHost) {
    if (!globalThis.__anywhereTransport) throw new HostError('HOST_UNAVAILABLE');
    defaultHost = createHost(globalThis.__anywhereTransport);
  }
  return defaultHost;
}
export const host = {
  ready: (options?: CallOptions) => injected().ready(options),
  invocation: { get: (options?: CallOptions) => injected().invocation.get(options) },
  context: { getCurrentApp: (options?: CallOptions) => injected().context.getCurrentApp(options) },
  clipboard: { readText: (options?: CallOptions) => injected().clipboard.readText(options), writeText: (text: string, options?: CallOptions) => injected().clipboard.writeText(text, options) },
  storage: {
    get: <T extends JsonValue = JsonValue>(key: string, options?: CallOptions) => injected().storage.get<T>(key, options),
    set: (key: string, value: JsonValue, options?: CallOptions) => injected().storage.set(key, value, options),
    remove: (key: string, options?: CallOptions) => injected().storage.remove(key, options),
  },
};
