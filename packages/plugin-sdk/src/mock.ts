// Development transport. Import this module in a browser test or a separate dev entry.
import type {HostTransport} from './client.js';
import type {
  Calls,
  PluginSnapshot,
  WebInput,
  WebInvocation,
} from './contract.js';
import {limits, protocol} from './contract.js';
import type {Language, Platform} from './environment.js';

const encoder = new TextEncoder();
type Scenario = 'normal' | 'revoked' | 'too-large' | 'cancel-confirmation';
type Request = {
  requestId: string;
  protocol: number;
  method: keyof Calls;
  params: Record<string, unknown>;
};
export const mockFile: WebInput = {
  kind: 'resources',
  source: 'finder_selection',
  items: [
    {
      handle: 'mock-file',
      name: 'sample.csv',
      kind: 'file',
      readable: true,
      previewable: true,
    },
  ],
};

export function createMock(initial: Partial<WebInvocation> = {}) {
  const storage = new Map<string, unknown>();
  const receivers = new Set<(frame: string) => void>();
  const upload = new Map<
    string,
    {bytes: number; sequence: number; text: string}
  >();
  const download = new Map<string, string[]>();
  let ticket = 0;
  let clipboard: string | null = null;
  let visible = true;
  let scenario: Scenario = 'normal';
  let state: PluginSnapshot = {
    version: 1,
    context: initial.input ?? {
      kind: 'text',
      source: 'selected_text',
      text: 'Sample selection',
    },
    input: '',
  };

  function emit(frame: unknown) {
    for (const receive of receivers) receive(JSON.stringify(frame));
  }

  function update(next: Partial<Omit<PluginSnapshot, 'version'>>) {
    state = {...state, ...next, version: state.version + 1};
    if (visible) emit({event: 'state.changed', version: state.version, state});
  }

  const transport: HostTransport = {
    async send(frame) {
      const request = JSON.parse(frame) as Request;
      const {requestId, method, params} = request;
      if (method === 'request.cancel') return;
      const fail = (code: string) => emit({requestId, error: {code}});
      const reply = (value: unknown) => {
        const encoded = JSON.stringify(value);
        if (encoder.encode(encoded).length <= limits.frameBytes - 128) {
          emit({requestId, result: {value}});
          return;
        }
        const bytes = encoder.encode(encoded).length;
        if (bytes > limits.transferBytes) return fail('TOO_LARGE');
        if (download.size) return fail('BUSY');
        const parts: string[] = [];
        // Up to four UTF-8 bytes per code point.
        let part: string[] = [];
        for (const character of encoded) {
          part.push(character);
          if (part.length === limits.chunkBytes / 4) {
            parts.push(part.join(''));
            part = [];
          }
        }
        if (part.length) parts.push(part.join(''));
        const name = `download_${++ticket}`;
        download.set(name, parts);
        emit({requestId, result: {transfer: {ticket: name, bytes}}});
      };
      if (!visible) return fail('INACTIVE');
      if (request.protocol !== protocol) return fail('INCOMPATIBLE_PROTOCOL');
      const business = ![
        'lifecycle.ready',
        'invocation.get',
        'state.get',
        'state.subscribe',
      ].includes(method);
      if (business && scenario === 'revoked') return fail('PERMISSION_DENIED');
      if (business && scenario === 'too-large') return fail('TOO_LARGE');
      const key = String(params.key);
      const handle = String(params.handle);
      const text = String(params.text ?? '');
      const access =
        state.context.kind === 'resources' &&
        state.context.items.some(item => item.handle === handle);
      switch (method) {
        case 'lifecycle.ready':
          return reply(null);
        case 'invocation.get':
          return reply({
            pluginId: 'dev.example.mock',
            commandId: 'open',
            entrypoint: 'workspace',
            capabilities: {
              urlOpen: true,
              resourceRead: true,
              previewOpen: true,
              windowControl: true,
            },
            ...initial,
            input: state.context,
          });
        case 'state.get':
        case 'state.subscribe':
          return reply(state);
        case 'context.get':
          return reply(state.context);
        case 'input.get':
          return reply(state.input);
        case 'input.set':
          update({input: text});
          return reply(null);
        case 'app.getSource':
        case 'app.getFrontmost':
          return reply({id: 'dev.example.mock-app', name: 'Mock App'});
        case 'clipboard.readText':
          return reply(clipboard);
        case 'clipboard.writeText':
          clipboard = text;
          return reply(null);
        case 'storage.get':
          return reply(storage.get(key) ?? null);
        case 'storage.set':
          storage.set(key, params.value);
          return reply(null);
        case 'storage.remove':
          storage.delete(key);
          return reply(null);
        case 'text.begin': {
          if (upload.size) return fail('BUSY');
          const bytes = Number(params.bytes);
          if (!Number.isSafeInteger(bytes) || bytes < 0)
            return fail('INVALID_REQUEST');
          if (bytes > limits.textBytes) return fail('TOO_LARGE');
          const name = `upload_${++ticket}`;
          upload.set(name, {bytes, sequence: 0, text: ''});
          return reply({ticket: name});
        }
        case 'text.chunk': {
          const target = upload.get(String(params.ticket));
          if (!target) return fail('EXPIRED');
          const chunkBytes = encoder.encode(text).length;
          if (
            target.sequence !== params.sequence ||
            chunkBytes > limits.chunkBytes ||
            encoder.encode(target.text).length + chunkBytes > target.bytes
          )
            return fail('INVALID_REQUEST');
          target.text += text;
          target.sequence++;
          return reply(null);
        }
        case 'text.commit': {
          const name = String(params.ticket);
          const target = upload.get(name);
          if (!target) return fail('EXPIRED');
          upload.delete(name);
          if (encoder.encode(target.text).length !== target.bytes)
            return fail('INVALID_REQUEST');
          clipboard = target.text;
          return reply(null);
        }
        case 'text.read': {
          const name = String(params.ticket);
          const chunks = download.get(name);
          const sequence = Number(params.sequence);
          const chunk = chunks?.[sequence];
          if (!chunks || chunk === undefined) return fail('EXPIRED');
          const done = sequence === chunks.length - 1;
          if (done) download.delete(name);
          return reply({text: chunk, sequence, done});
        }
        case 'text.cancel':
          upload.delete(String(params.ticket));
          download.delete(String(params.ticket));
          return reply(null);
        case 'url.open':
          return scenario === 'cancel-confirmation'
            ? fail('USER_CANCELLED')
            : reply(null);
        case 'resources.stat':
          return access
            ? reply({handle, name: 'sample.csv', kind: 'file', bytes: 14})
            : fail('EXPIRED');
        case 'resources.readText': {
          if (!access) return fail('EXPIRED');
          const csv = 'name,age\nLin,4';
          return Number(params.maxBytes ?? limits.textBytes) <
            encoder.encode(csv).length
            ? fail('TOO_LARGE')
            : reply(csv);
        }
        case 'preview.open':
          return access ? reply(null) : fail('EXPIRED');
        case 'window.back':
        case 'window.close':
          return reply(null);
        default:
          return fail('UNSUPPORTED_METHOD');
      }
    },
    subscribe(receive) {
      receivers.add(receive);
      return () => receivers.delete(receive);
    },
  };

  return {
    transport,
    get clipboard() {
      return clipboard;
    },
    setClipboard(text: string | null) {
      clipboard = text;
    },
    setContext(context: WebInput) {
      update({context});
    },
    setScenario(next: Scenario) {
      scenario = next;
    },
    setVisible(next: boolean) {
      if (visible === next) return;
      visible = next;
      if (!visible) {
        upload.clear();
        download.clear();
      }
      globalThis.dispatchEvent?.(
        new CustomEvent('anywhere:visibility', {detail: visible}),
      );
    },
    setPlatform(platform: Platform) {
      globalThis.__anywherePlatform = platform;
      globalThis.dispatchEvent?.(
        new CustomEvent('anywhere:platform', {detail: platform}),
      );
    },
    setLanguage(language: Language) {
      globalThis.__anywhereLanguage = language;
      if (typeof document !== 'undefined')
        document.documentElement.lang = language;
      globalThis.dispatchEvent?.(
        new CustomEvent('anywhere:language', {detail: language}),
      );
    },
    setTheme(theme: 'light' | 'dark') {
      if (typeof document !== 'undefined') {
        document.documentElement.dataset.anywhereMockTheme = theme;
        document.documentElement.style.colorScheme = theme;
      }
    },
  };
}

export function mountMockPanel(mock: ReturnType<typeof createMock>) {
  const panel = document.createElement('aside');
  panel.className = 'aw-card';
  panel.setAttribute('aria-label', 'Anywhere development controls');
  function select<T extends string>(
    label: string,
    values: readonly T[],
    change: (value: T) => void,
  ) {
    const field = document.createElement('label');
    field.append(label + ' ');
    const input = document.createElement('select');
    for (const value of values) input.add(new Option(value, value));
    input.addEventListener('change', () => change(input.value as T));
    field.append(input);
    panel.append(field, ' ');
  }
  select('Platform', ['macos', 'windows'], mock.setPlatform);
  select('Language', ['en', 'zh'], mock.setLanguage);
  mock.setTheme('dark');
  select('Theme', ['dark', 'light'], mock.setTheme);
  select(
    'Scenario',
    ['normal', 'revoked', 'too-large', 'cancel-confirmation'],
    mock.setScenario,
  );
  select('Visibility', ['visible', 'hidden'], value =>
    mock.setVisible(value === 'visible'),
  );
  select('Selection', ['text', 'file'], value =>
    mock.setContext(
      value === 'file'
        ? mockFile
        : {kind: 'text', source: 'selected_text', text: 'Sample selection'},
    ),
  );
  document.body.prepend(panel);
  return () => panel.remove();
}
