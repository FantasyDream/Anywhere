import {createServer} from 'node:http';
import {realpath, readFile} from 'node:fs/promises';
import {dirname, extname, join, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {readManifest} from './manifest.mjs';

const project = process.cwd();
const manifest = await readManifest(project);
const assets = await realpath(join(project, 'dist'));
const sdk = await realpath(
  join(dirname(fileURLToPath(import.meta.url)), '../dist'),
);
const styles = await realpath(join(sdk, '../styles'));
const palettes = new URL('mock-palettes.css', import.meta.url);
const types = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
};
const mock = `
import {createMock, mountMockPanel} from '/__sdk/mock.js';
const mock = createMock({pluginId: ${JSON.stringify(manifest.id)}});
globalThis.__anywhereTransport = mock.transport;
mountMockPanel(mock);
`;
const server = createServer(async (request, response) => {
  try {
    if (request.method !== 'GET') {
      response.writeHead(405).end();
      return;
    }
    let path = decodeURIComponent(
      new URL(request.url, 'http://localhost').pathname,
    );
    if (path === '/__mock-palettes.css') {
      response
        .writeHead(200, {
          'Content-Type': 'text/css',
          'Cache-Control': 'no-store',
        })
        .end(await readFile(palettes));
      return;
    }
    if (path === '/__mock.js') {
      response
        .writeHead(200, {
          'Content-Type': 'text/javascript',
          'Cache-Control': 'no-store',
        })
        .end(mock);
      return;
    }
    if (path === '/') path = '/' + manifest.entry.slice('dist/'.length);
    const [root, prefix] = path.startsWith('/__sdk/')
      ? [sdk, '/__sdk/']
      : path.startsWith('/__styles/')
        ? [styles, '/__styles/']
        : [assets, '/'];
    const file = await realpath(join(root, path.slice(prefix.length)));
    if (!file.startsWith(root + sep))
      throw new Error('Path outside dev assets');
    const extension = extname(file);
    let body = await readFile(file);
    if (extension === '.html') {
      body = body
        .toString('utf8')
        .replace(
          /<\/head>/i,
          '<link rel="stylesheet" href="/__styles/tokens.css"><link rel="stylesheet" href="/__styles/controls.css"><link rel="stylesheet" href="/__mock-palettes.css">$&',
        )
        .replace(
          /<body[^>]*>/i,
          '$&<script type="module" src="/__mock.js"></script>',
        );
    }
    response
      .writeHead(200, {
        'Content-Type': types[extension] ?? 'application/octet-stream',
        'Cache-Control': 'no-store',
        'X-Content-Type-Options': 'nosniff',
      })
      .end(body);
  } catch {
    response.writeHead(404).end('Not found');
  }
});
server.listen(0, '127.0.0.1', () => {
  process.stdout.write(
    `Anywhere Mock at http://127.0.0.1:${server.address().port}\n`,
  );
});
