# Background Context example

Build with `npm ci && npm run build`, then use the SDK packer and import the
`.awplugin` through Settings → Web tools. `scripts/build-web-examples.py` also
builds this example. No WebView, CSS, native code or runtime TypeScript compiler
is involved.

When the Context graph receives a JSON text object/array, `resolve` contributes
a package-owned summary. Its separate **Copy JSON Summary** Action asks the Host
for clipboard consent, copies the summary, then exits. Ordinary non-JSON text
produces no Context or Action. Automatic resolvers cannot write the clipboard
or change the query, even when the package declares those grants for a command.
