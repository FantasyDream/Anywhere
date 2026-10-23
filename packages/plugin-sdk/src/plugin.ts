import {createHost, HostError} from './client.js';
import {lifecycle} from './lifecycle.js';
import type {Invocation, JsonValue, CallOptions} from './client.js';

export interface PluginOptions {
  /** Called once with this launch's input, after the Host connection is ready. */
  onLaunch?: (
    invocation: Invocation,
    context: {signal: AbortSignal},
  ) => void | Promise<void>;
  onError?: (error: unknown) => void;
  onDispose?: () => void;
}

/** Accessible fallback; tools may supply their own business-specific error UI. */
function showError(error: unknown) {
  if (typeof document === 'undefined') return;
  let notice = document.getElementById('anywhere-plugin-error');
  if (!notice) {
    notice = document.createElement('aside');
    notice.id = 'anywhere-plugin-error';
    notice.className = 'aw-workspace-notice';
    notice.setAttribute('role', 'alert');
    document.body.prepend(notice);
  }
  notice.textContent = `This operation could not be completed (${error instanceof HostError ? error.code : 'TOOL_ERROR'}).`;
}

/** Owns the Host connection, startup, retained-page resume and pending requests.
 * Call after rendering the page. Uses ordinary HTML / any frontend framework;
 * Host styling is injected automatically and no UI description language is used.
 */
export function definePlugin(options: PluginOptions = {}) {
  let client: ReturnType<typeof createHost> | undefined;
  let active = true,
    disposed = false,
    delivered = false,
    generation = 0;
  const lifetime = new AbortController();
  let settleReady!: () => void;
  const ready = new Promise<void>(resolve => {
    settleReady = resolve;
  });
  function report(error: unknown) {
    if (!active || disposed) return;
    try {
      (options.onError ?? showError)(error);
    } catch {
      showError(error);
    }
  }
  function current() {
    if (!active || disposed) throw new HostError('INACTIVE');
    if (!client) {
      if (!globalThis.__anywhereTransport)
        throw new HostError('HOST_UNAVAILABLE');
      client = createHost(globalThis.__anywhereTransport);
    }
    return client;
  }
  // Stable facade: handlers created on first render also use the resumed binding.
  const host = {
    context: {
      getCurrentApp: async (opts?: CallOptions) =>
        current().context.getCurrentApp(opts),
    },
    clipboard: {
      readText: async (opts?: CallOptions) =>
        current().clipboard.readText(opts),
      writeText: async (text: string, opts?: CallOptions) =>
        current().clipboard.writeText(text, opts),
    },
    storage: {
      get: async <T extends JsonValue = JsonValue>(
        key: string,
        opts?: CallOptions,
      ) => current().storage.get<T>(key, opts),
      set: async (key: string, value: JsonValue, opts?: CallOptions) =>
        current().storage.set(key, value, opts),
      remove: async (key: string, opts?: CallOptions) =>
        current().storage.remove(key, opts),
    },
  };
  async function connect() {
    const ticket = ++generation;
    const isCurrent = () => active && !disposed && ticket === generation;
    try {
      const api = current();
      await api.ready();
      if (!isCurrent()) return;
      if (!delivered) {
        const invocation = await api.invocation.get();
        if (!isCurrent()) return;
        // A failed business callback must not be run twice on a subsequent wake.
        delivered = true;
        await options.onLaunch?.(invocation, {signal: lifetime.signal});
      }
    } catch (error) {
      if (isCurrent()) report(error);
    } finally {
      if (isCurrent()) settleReady();
    }
  }
  const stop = lifecycle.onVisibility(visible => {
    if (disposed || active === visible) return;
    active = visible;
    if (visible) void connect();
    else {
      ++generation;
      client?.dispose();
      client = undefined;
    }
  });
  function dispose() {
    if (disposed) return;
    disposed = true;
    active = false;
    ++generation;
    stop();
    globalThis.removeEventListener('pagehide', dispose);
    lifetime.abort();
    client?.dispose();
    client = undefined;
    settleReady();
    try {
      options.onDispose?.();
    } catch {
      /* Teardown must complete. */
    }
  }
  globalThis.addEventListener('pagehide', dispose, {once: true});
  // Let callers finish rendering / attaching handlers before acknowledging ready.
  queueMicrotask(() => {
    if (!disposed) void connect();
  });
  return {host, ready, dispose};
}
