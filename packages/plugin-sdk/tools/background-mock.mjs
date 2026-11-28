import vm from 'node:vm';

/**
 * Run a bundled, trusted development fixture in a bare ECMAScript context.
 * The broker, its permissions and all input data are created inside the context.
 */
export async function runBackgroundMock({
  source,
  entrypoint,
  role,
  context,
  invocation = null,
  permissions,
  methods,
  urlOpen = 'cancel',
}) {
  if (typeof vm.SourceTextModule !== 'function') {
    throw new Error(
      'runBackgroundMock requires Node --experimental-vm-modules',
    );
  }
  const realm = vm.createContext(Object.create(null), {
    codeGeneration: {strings: false, wasm: false},
  });
  const timeout = 250;
  const module = new vm.SourceTextModule(source, {context: realm});
  const config = JSON.stringify({context, role, permissions, methods, urlOpen});
  vm.runInContext(
    `
    for (const name of [
      'URL', 'TextEncoder', 'TextDecoder', 'fetch', 'setTimeout', 'console',
      'Intl', 'WebAssembly', 'Temporal', 'SharedArrayBuffer', 'Atomics',
    ]) {
      delete globalThis[name];
    }
    (() => {
      const {context, role, permissions, methods, urlOpen} = JSON.parse(${JSON.stringify(config)});
      const data = new Map();
      const uploads = new Map();
      const snapshot = {version: 1, context, input: permissions.includes('input.read') ? 'sample' : null};
      let clipboard = null;
      let calls = 0;
      let businessCalls = 0;
      let ticket = 0;
      const transportMethods = new Set(['text.begin', 'text.chunk', 'text.commit', 'text.read', 'text.cancel']);
      const utf8Bytes = text => {
        let len = 0;
        for (let i = 0; i < text.length; i++) {
          const code = text.charCodeAt(i);
          if (code < 0x80) len += 1;
          else if (code < 0x800) len += 2;
          else if (code >= 0xd800 && code <= 0xdbff) { len += 4; i++; }
          else len += 3;
        }
        return len;
      };
      const unsupported = code => JSON.stringify({error: {code}});
      globalThis.__mockCalls = [];
      globalThis.__anywhereInvoke = (method, raw) => {
        if (++calls > 1024) return unsupported('QUOTA_EXCEEDED');
        if (!transportMethods.has(method) && ++businessCalls > 128) return unsupported('QUOTA_EXCEEDED');
        if (!Object.hasOwn(methods, method)) return unsupported('UNSUPPORTED_METHOD');
        const policy = methods[method];
        if (!policy.roles.includes(role) || (policy.permission && !permissions.includes(policy.permission))) {
          __mockCalls.push(method);
          return unsupported('PERMISSION_DENIED');
        }
        const params = JSON.parse(raw);
        const ok = value => JSON.stringify({value});
        switch (method) {
          case 'state.get':
          case 'state.subscribe': return ok(snapshot);
          case 'context.get': return ok(snapshot.context);
          case 'input.get': return ok(snapshot.input);
          case 'input.set': snapshot.input = params.text; return ok(null);
          case 'app.getSource':
          case 'app.getFrontmost': return ok(null);
          case 'clipboard.readText': return ok(clipboard);
          case 'clipboard.writeText': clipboard = params.text; return ok(null);
          case 'text.begin': {
            if (uploads.size) return unsupported('BUSY');
            const bytes = params.bytes;
            if (!Number.isSafeInteger(bytes) || bytes < 0) return unsupported('INVALID_REQUEST');
            if (bytes > 1024 * 1024) return unsupported('TOO_LARGE');
            const id = (++ticket).toString(16).padStart(64, '0');
            uploads.set(id, {bytes, sequence: 0, text: ''});
            return ok({ticket: id});
          }
          case 'text.chunk': {
            const target = uploads.get(String(params.ticket));
            if (!target) return unsupported('EXPIRED');
            const chunk = String(params.text ?? '');
            const chunkBytes = utf8Bytes(chunk);
            if (target.sequence !== params.sequence || chunkBytes > 4096 || utf8Bytes(target.text) + chunkBytes > target.bytes) {
              return unsupported('INVALID_REQUEST');
            }
            target.text += chunk;
            target.sequence++;
            return ok(null);
          }
          case 'text.commit': {
            const id = String(params.ticket);
            const target = uploads.get(id);
            if (!target) return unsupported('EXPIRED');
            uploads.delete(id);
            if (utf8Bytes(target.text) !== target.bytes) return unsupported('INVALID_REQUEST');
            clipboard = target.text;
            return ok(null);
          }
          case 'text.cancel': uploads.delete(String(params.ticket)); return ok(null);
          case 'storage.get': return ok(data.get(params.key) ?? null);
          case 'storage.set': data.set(params.key, params.value); return ok(null);
          case 'storage.remove': data.delete(params.key); return ok(null);
          case 'url.open': return urlOpen === 'allow' ? ok(null) : unsupported('USER_CANCELLED');
          case 'resources.stat':
          case 'resources.readText': {
            const file = context.kind === 'resources' && context.items.find(file => file.handle === params.handle);
            if (!file) return unsupported('EXPIRED');
            const contents = 'name,age\\nLin,4';
            if (method === 'resources.stat') return ok({handle: file.handle, name: file.name, kind: 'file', bytes: contents.length});
            const limit = params.maxBytes ?? (1024 * 1024);
            if (!Number.isInteger(limit) || limit < 1 || limit > 1024 * 1024) return unsupported('INVALID_REQUEST');
            return contents.length <= limit ? ok(contents) : unsupported('TOO_LARGE');
          }
          default: return unsupported('UNSUPPORTED_METHOD');
        }
      };
    })();
  `,
    realm,
    {timeout},
  );
  await module.link(() => {
    throw new Error('Bundle all imports into the background entry');
  });
  await module.evaluate({timeout});
  realm.__mockExport = module.namespace[entrypoint];
  if (typeof realm.__mockExport !== 'function')
    throw new Error(`Missing background export: ${entrypoint}`);
  realm.__mockInvocation = JSON.stringify(invocation);
  const output = await vm.runInContext(
    'Promise.resolve(__mockExport(JSON.parse(__mockInvocation))).then(value => JSON.stringify({value: value ?? null, denied: __mockCalls}))',
    realm,
    {timeout},
  );
  if (Buffer.byteLength(output) > 256 * 1024)
    throw new Error('Background output exceeds 256 KiB');
  return JSON.parse(output);
}
