import {protocol, limits, errorCodes} from './contract.js';
import type {
  Calls,
  JsonValue,
  WebInvocation as Invocation,
  WebCurrentApp as CurrentApp,
  PluginSnapshot,
} from './contract.js';
export type {
  JsonValue,
  WebInput as Input,
  WebInvocation as Invocation,
  WebCurrentApp as CurrentApp,
  PluginSnapshot,
} from './contract.js';

export interface HostTransport {
  send(frame: string): void | Promise<void>;
  subscribe(receive: (frame: string) => void): () => void;
}
import type {CallOptions} from './api.js';
export type {CallOptions} from './api.js';
import {HostError} from './errors.js';
export {HostError} from './errors.js';

type Body = {value: unknown} | {transfer: {ticket: string; bytes: number}};
type Job = {
  id: string;
  frame: string;
  timeout: number;
  options?: CallOptions;
  resolve: (value: Body) => void;
  reject: (error: unknown) => void;
  timer?: ReturnType<typeof setTimeout>;
  abort?: () => void;
};
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true});
function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** No generic invoke API is exported. The injected transport carries no authority from JS. */
export function createHost(transport: HostTransport) {
  const random = crypto.getRandomValues(new Uint32Array(4));
  const prefix = Array.from(random, n => n.toString(16).padStart(8, '0')).join(
    '',
  );
  let sequence = 0,
    disposed = false;
  let snapshot: PluginSnapshot | undefined;
  let reading: Promise<PluginSnapshot> | undefined;
  let targetVersion = 0,
    refresh = 0,
    refreshed = 0,
    subscribed = false;
  let cancelStateRetry: (() => void) | undefined;
  const contextListeners = new Set<
    (context: PluginSnapshot['context']) => void
  >();
  const inputListeners = new Set<(input: string) => void>();
  function accept(state: PluginSnapshot) {
    if (disposed || (snapshot && state.version <= snapshot.version)) return;
    const previous = snapshot;
    snapshot = state;
    if (
      !previous ||
      JSON.stringify(previous.context) !== JSON.stringify(state.context)
    )
      for (const listener of contextListeners) {
        try {
          listener(state.context);
        } catch {
          /* One subscriber must not interrupt other callbacks. */
        }
      }
    if (state.input !== null && state.input !== previous?.input)
      for (const listener of inputListeners) {
        try {
          listener(state.input);
        } catch {
          /* One subscriber must not interrupt other callbacks. */
        }
      }
  }
  function fresh() {
    return (
      snapshot !== undefined &&
      snapshot.version >= targetVersion &&
      refreshed === refresh
    );
  }
  async function readState(): Promise<PluginSnapshot> {
    const deadline = Date.now() + limits.timeoutMs;
    while (!fresh()) {
      if (disposed) throw new HostError('INACTIVE');
      if (Date.now() >= deadline) throw new HostError('TIMEOUT');
      const revision = refresh;
      try {
        const value = await invoke<
          'state.subscribe' | 'state.get',
          PluginSnapshot
        >(subscribed ? 'state.get' : 'state.subscribe', {});
        subscribed = true;
        accept(value);
        refreshed = revision;
      } catch (error) {
        if (!(error instanceof HostError) || error.code !== 'BUSY') throw error;
        // Another bounded transfer may own the document. Retry only reads;
        // the client lifetime owns both the delay and the active request.
        await new Promise<void>((resolve, reject) => {
          const timer = setTimeout(() => {
            cancelStateRetry = undefined;
            resolve();
          }, 50);
          cancelStateRetry = () => {
            clearTimeout(timer);
            cancelStateRetry = undefined;
            reject(new HostError('INACTIVE'));
          };
          if (disposed) cancelStateRetry();
        });
      }
    }
    return snapshot!;
  }
  function state(): Promise<PluginSnapshot> {
    if (disposed) return Promise.reject(new HostError('INACTIVE'));
    if (fresh()) return Promise.resolve(snapshot!);
    return (reading ??= Promise.resolve()
      .then(readState)
      .finally(() => {
        reading = undefined;
      }));
  }
  const pending = new Map<string, Job>();
  const queue: Job[] = [];
  const id = () => `${prefix}_${++sequence}`;
  function send(frame: string, failed: (error: unknown) => void) {
    try {
      Promise.resolve(transport.send(frame)).catch(failed);
    } catch (error) {
      failed(error);
    }
  }
  function cancel(requestId: string) {
    // Best effort cancellation is not a retry of the original side effect.
    send(
      JSON.stringify({
        protocol,
        requestId: id(),
        method: 'request.cancel',
        params: {request_id: requestId},
      }),
      () => {},
    );
  }
  function settle(job: Job, error?: unknown, value?: Body) {
    const queued = queue.indexOf(job);
    const active = pending.delete(job.id);
    if (!active && queued < 0) return;
    if (queued >= 0) queue.splice(queued, 1);
    clearTimeout(job.timer);
    if (job.abort) job.options?.signal?.removeEventListener('abort', job.abort);
    if (error) job.reject(error);
    else job.resolve(value!);
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
    if (
      disposed ||
      frame.length > limits.frameBytes ||
      encoder.encode(frame).length > limits.frameBytes
    )
      return;
    let message: unknown;
    try {
      message = JSON.parse(frame);
    } catch {
      return;
    }
    if (!object(message)) return;
    if (
      message.event === 'state.changed' &&
      typeof message.version === 'number'
    ) {
      if (object(message.state))
        accept(message.state as unknown as PluginSnapshot);
      else if (!snapshot || message.version > snapshot.version) {
        targetVersion = Math.max(targetVersion, message.version);
        void state().catch(() => {});
      }
      return;
    }
    if (typeof message.requestId !== 'string') return;
    const job = pending.get(message.requestId);
    if (!job) return;
    if ('error' in message && !('result' in message) && object(message.error)) {
      const code = message.error.code;
      settle(
        job,
        new HostError(
          typeof code === 'string' &&
            (errorCodes as readonly string[]).includes(code)
            ? code
            : 'HOST_FAILURE',
        ),
      );
    } else if (
      'result' in message &&
      !('error' in message) &&
      object(message.result)
    ) {
      const result = message.result;
      if ('value' in result && !('transfer' in result))
        settle(job, undefined, {value: result.value});
      else if (
        'transfer' in result &&
        !('value' in result) &&
        object(result.transfer) &&
        typeof result.transfer.ticket === 'string' &&
        /^[a-f0-9]{64}$/.test(result.transfer.ticket) &&
        Number.isSafeInteger(result.transfer.bytes) &&
        (result.transfer.bytes as number) > 0 &&
        (result.transfer.bytes as number) <= limits.transferBytes
      ) {
        settle(job, undefined, {
          transfer: result.transfer as {ticket: string; bytes: number},
        });
      } else settle(job, new HostError('INVALID_RESPONSE'));
    } else settle(job, new HostError('INVALID_RESPONSE'));
  });
  function raw<M extends keyof Calls>(
    method: M,
    params: Calls[M],
    options?: CallOptions,
    timeout: number = limits.timeoutMs,
  ): Promise<Body> {
    if (disposed) return Promise.reject(new HostError('INACTIVE'));
    if (options?.signal?.aborted)
      return Promise.reject(new HostError('CANCELLED'));
    if (pending.size >= limits.running && queue.length >= limits.queue)
      return Promise.reject(new HostError('BUSY'));
    const requestId = id();
    let frame: string;
    try {
      frame = JSON.stringify({protocol, requestId, method, params});
    } catch {
      return Promise.reject(new HostError('INVALID_REQUEST'));
    }
    if (encoder.encode(frame).length > limits.frameBytes)
      return Promise.reject(new HostError('TOO_LARGE'));
    return new Promise((resolve, reject) => {
      const job: Job = {
        id: requestId,
        frame,
        options,
        timeout,
        resolve,
        reject,
      };
      job.abort = () => {
        const sent = pending.has(job.id);
        settle(job, new HostError('CANCELLED'));
        if (sent) cancel(job.id);
      };
      options?.signal?.addEventListener('abort', job.abort, {once: true});
      queue.push(job);
      pump();
    });
  }
  async function invoke<M extends keyof Calls, T = unknown>(
    method: M,
    params: Calls[M],
    options?: CallOptions,
    timeout?: number,
  ): Promise<T> {
    const body = await raw(method, params, options, timeout);
    if ('value' in body) return body.value as T;
    const {ticket, bytes} = body.transfer;
    const chunks: string[] = [];
    let count = 0;
    try {
      for (let sequence = 0; ; sequence++) {
        const part = await raw('text.read', {ticket, sequence}, options);
        if (
          !('value' in part) ||
          !object(part.value) ||
          typeof part.value.text !== 'string' ||
          part.value.sequence !== sequence ||
          typeof part.value.done !== 'boolean'
        )
          throw new HostError('INVALID_RESPONSE');
        const size = encoder.encode(part.value.text).length;
        if (size > limits.chunkBytes || size === 0 || (count += size) > bytes)
          throw new HostError('INVALID_RESPONSE');
        chunks.push(part.value.text);
        if (part.value.done) break;
      }
      if (count !== bytes) throw new HostError('INVALID_RESPONSE');
      return JSON.parse(chunks.join('')) as T;
    } catch (error) {
      void raw('text.cancel', {ticket}).catch(() => {});
      throw error;
    }
  }
  async function writeText(text: string, options?: CallOptions): Promise<void> {
    const bytes = encoder.encode(text);
    if (bytes.length > limits.textBytes) throw new HostError('TOO_LARGE');
    if (decoder.decode(bytes) !== text) throw new HostError('INVALID_REQUEST');
    // Reserve framing overhead so escaped text automatically takes the chunk path.
    if (
      bytes.length + 256 <= limits.frameBytes &&
      encoder.encode(JSON.stringify({text})).length + 256 <= limits.frameBytes
    ) {
      return invoke('clipboard.writeText', {text}, options, limits.timeoutMs);
    }
    const {ticket} = await invoke<'text.begin', {ticket: string}>(
      'text.begin',
      {bytes: bytes.length},
      options,
    );
    try {
      for (let offset = 0, sequence = 0; offset < bytes.length; sequence++) {
        let end = Math.min(offset + limits.chunkBytes, bytes.length);
        while (end < bytes.length && (bytes[end]! & 0xc0) === 0x80) end--;
        await invoke(
          'text.chunk',
          {ticket, sequence, text: decoder.decode(bytes.subarray(offset, end))},
          options,
        );
        offset = end;
      }
      await invoke('text.commit', {ticket}, options);
    } catch (error) {
      void raw('text.cancel', {ticket}).catch(() => {});
      throw error;
    }
  }
  return {
    ready: (options?: CallOptions) =>
      invoke<'lifecycle.ready', void>('lifecycle.ready', {}, options),
    invocation: {
      get: (options?: CallOptions) =>
        invoke<'invocation.get', Invocation>('invocation.get', {}, options),
    },
    context: {
      get: async () => (await state()).context,
      onChange(callback: (context: PluginSnapshot['context']) => void) {
        let delivered: number | undefined;
        const receive = (context: PluginSnapshot['context']) => {
          if (delivered === snapshot?.version) return;
          delivered = snapshot?.version;
          callback(context);
        };
        contextListeners.add(receive);
        void state()
          .then(() => {
            if (contextListeners.has(receive) && snapshot)
              receive(snapshot.context);
          })
          .catch(() => {});
        return () => {
          contextListeners.delete(receive);
        };
      },
      getCurrentApp: (options?: CallOptions) =>
        invoke<'context.getCurrentApp', CurrentApp | null>(
          'context.getCurrentApp',
          {},
          options,
        ),
    },
    input: {
      get: async () => {
        const value = (await state()).input;
        if (value === null) throw new HostError('PERMISSION_DENIED');
        return value;
      },
      set: async (text: string, options?: CallOptions) => {
        await invoke<'input.set', void>('input.set', {text}, options);
        // A successful write precedes its pushed event. Fence any read already
        // in flight without turning a later read failure into a failed write.
        ++refresh;
      },
      onChange(callback: (input: string) => void) {
        let delivered: number | undefined;
        const receive = (input: string) => {
          if (delivered === snapshot?.version) return;
          delivered = snapshot?.version;
          callback(input);
        };
        inputListeners.add(receive);
        void state()
          .then(() => {
            if (
              inputListeners.has(receive) &&
              snapshot &&
              snapshot.input !== null
            )
              receive(snapshot.input);
          })
          .catch(() => {});
        return () => {
          inputListeners.delete(receive);
        };
      },
    },
    app: {
      getSource: (options?: CallOptions) =>
        invoke<'app.getSource', CurrentApp | null>(
          'app.getSource',
          {},
          options,
        ),
      getFrontmost: (options?: CallOptions) =>
        invoke<'app.getFrontmost', CurrentApp | null>(
          'app.getFrontmost',
          {},
          options,
        ),
    },
    clipboard: {
      readText: (options?: CallOptions) =>
        invoke<'clipboard.readText', string | null>(
          'clipboard.readText',
          {},
          options,
        ),
      writeText,
    },
    storage: {
      get: <T extends JsonValue = JsonValue>(
        key: string,
        options?: CallOptions,
      ) => invoke<'storage.get', T | null>('storage.get', {key}, options),
      set: (key: string, value: JsonValue, options?: CallOptions) =>
        invoke<'storage.set', void>('storage.set', {key, value}, options),
      remove: (key: string, options?: CallOptions) =>
        invoke<'storage.remove', void>('storage.remove', {key}, options),
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      cancelStateRetry?.();
      unsubscribe();
      contextListeners.clear();
      inputListeners.clear();
      snapshot = undefined;
      const sent = [...pending.keys()];
      for (const job of [...queue, ...pending.values()])
        settle(job, new HostError('INACTIVE'));
      for (const requestId of sent) cancel(requestId);
    },
  };
}

declare global {
  var __anywhereTransport: HostTransport | undefined;
}
let defaultHost: ReturnType<typeof createHost> | undefined;
function injected() {
  if (!defaultHost) {
    if (!globalThis.__anywhereTransport)
      throw new HostError('HOST_UNAVAILABLE');
    defaultHost = createHost(globalThis.__anywhereTransport);
  }
  return defaultHost;
}
export const host = {
  ready: (options?: CallOptions) => injected().ready(options),
  invocation: {
    get: (options?: CallOptions) => injected().invocation.get(options),
  },
  context: {
    get: () => injected().context.get(),
    onChange: (callback: (context: PluginSnapshot['context']) => void) =>
      injected().context.onChange(callback),
    getCurrentApp: (options?: CallOptions) =>
      injected().context.getCurrentApp(options),
  },
  input: {
    get: () => injected().input.get(),
    set: (text: string, options?: CallOptions) =>
      injected().input.set(text, options),
    onChange: (callback: (input: string) => void) =>
      injected().input.onChange(callback),
  },
  app: {
    getSource: (options?: CallOptions) => injected().app.getSource(options),
    getFrontmost: (options?: CallOptions) =>
      injected().app.getFrontmost(options),
  },
  clipboard: {
    readText: (options?: CallOptions) => injected().clipboard.readText(options),
    writeText: (text: string, options?: CallOptions) =>
      injected().clipboard.writeText(text, options),
  },
  storage: {
    get: <T extends JsonValue = JsonValue>(
      key: string,
      options?: CallOptions,
    ) => injected().storage.get<T>(key, options),
    set: (key: string, value: JsonValue, options?: CallOptions) =>
      injected().storage.set(key, value, options),
    remove: (key: string, options?: CallOptions) =>
      injected().storage.remove(key, options),
  },
};
