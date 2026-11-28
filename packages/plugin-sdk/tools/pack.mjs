import {randomUUID} from 'node:crypto';
import {
  lstat,
  mkdir,
  readdir,
  readFile,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import {basename, dirname, join, relative, resolve, sep} from 'node:path';
import {crc32, deflateRawSync} from 'node:zlib';
import {check, checkAssetPath, maxManifestBytes} from './manifest.mjs';

const MiB = 1024 * 1024;
const maxAsset = 16 * MiB;
const maxArchive = 20 * MiB;
const maxExpanded = 80 * MiB;
const maxEntries = 2000;
const extensions =
  /\.(html?|m?js|css|json|txt|md|png|jpe?g|gif|webp|ico|svg|woff2?|ttf|otf|map)$/;
const caseFold = JSON.parse(
  await readFile(new URL('./unicode-case-fold.json', import.meta.url), 'utf8'),
);
const assetKey = path =>
  Array.from(path.normalize('NFC'), character =>
    Object.hasOwn(caseFold, character) ? caseFold[character] : character,
  )
    .join('')
    .normalize('NFC');
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const utf8 = 0x0800;
const dosDate = ((2026 - 1980) << 9) | (1 << 5) | 1;

async function collect(project, manifest) {
  const archiveManifest = {...manifest};
  delete archiveManifest.$schema;
  const bytes = Buffer.from(JSON.stringify(archiveManifest, null, 2) + '\n');
  check(
    bytes.length <= maxManifestBytes,
    'Normalized manifest exceeds 128 KiB',
  );
  const files = [{name: 'manifest.json', bytes}];
  let total = bytes.length;
  const names = new Set();
  async function walk(relativePath) {
    const path = join(project, relativePath);
    const info = await lstat(path).catch(error => {
      if (error.code === 'ENOENT')
        throw new Error(
          `Missing built asset ${relativePath}. Run npm run build first.`,
        );
      throw error;
    });
    check(
      info.isDirectory() || info.isFile(),
      `Not a regular asset: ${relativePath}`,
    );
    checkAssetPath(relativePath);
    check(names.size < maxEntries * 16, 'Too many runtime asset paths');
    const key = assetKey(relativePath);
    check(!names.has(key), `Duplicate portable asset path: ${relativePath}`);
    names.add(key);
    if (info.isDirectory()) {
      for (const child of await readdir(path)) {
        if (child !== '.DS_Store') await walk(`${relativePath}/${child}`);
      }
      return;
    }
    check(
      extensions.test(relativePath),
      `Unsupported asset extension: ${relativePath}`,
    );
    check(
      files.length < maxEntries &&
        info.size <= maxAsset &&
        total + info.size <= maxExpanded,
      'Runtime asset budget exceeded',
    );
    const data = await readFile(path);
    check(
      data.length === info.size,
      `Asset changed while reading: ${relativePath}`,
    );
    total += data.length;
    files.push({name: relativePath, bytes: data});
  }
  await walk('dist');
  for (const entry of [
    manifest.entry,
    manifest.background?.entry,
    manifest.icon,
  ]) {
    if (entry)
      check(
        files.some(file => file.name === entry),
        `Missing built asset: ${entry}`,
      );
  }
  if (manifest.icon) {
    const icon = files.find(file => file.name === manifest.icon).bytes;
    check(
      icon.length >= 33 &&
        icon.length <= 256 * 1024 &&
        icon.subarray(0, 8).equals(pngSignature) &&
        icon.toString('ascii', 12, 16) === 'IHDR' &&
        [icon.readUInt32BE(16), icon.readUInt32BE(20)].every(
          size => size >= 1 && size <= 512,
        ),
      'Expected a PNG icon up to 512 × 512 and 256 KiB',
    );
  }
  return files;
}

function zip(files) {
  const entries = files.map(file => ({...file, path: Buffer.from(file.name)}));
  entries.sort((left, right) => Buffer.compare(left.path, right.path));
  const local = [];
  const central = [];
  let offset = 0;
  let directorySize = 0;
  for (const {path, bytes} of entries) {
    const compressed = deflateRawSync(bytes, {level: 9});
    const useDeflate =
      compressed.length < bytes.length &&
      bytes.length <= compressed.length * 200;
    const stored = useDeflate ? compressed : bytes;
    const method = useDeflate ? 8 : 0;
    const checksum = crc32(bytes);

    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50, 0);
    header.writeUInt16LE(20, 4);
    header.writeUInt16LE(utf8, 6);
    header.writeUInt16LE(method, 8);
    header.writeUInt16LE(dosDate, 12);
    header.writeUInt32LE(checksum, 14);
    header.writeUInt32LE(stored.length, 18);
    header.writeUInt32LE(bytes.length, 22);
    header.writeUInt16LE(path.length, 26);

    const record = Buffer.alloc(46);
    record.writeUInt32LE(0x02014b50, 0);
    record.writeUInt16LE(0x0314, 4);
    header.copy(record, 6, 4, 28);
    record.writeUInt32LE((0o100644 << 16) >>> 0, 38);
    record.writeUInt32LE(offset, 42);

    offset += header.length + path.length + stored.length;
    directorySize += record.length + path.length;
    check(offset + directorySize + 22 <= maxArchive, 'Package exceeds 20 MiB');
    local.push(header, path, stored);
    central.push(record, path);
  }
  const trailer = Buffer.alloc(22);
  trailer.writeUInt32LE(0x06054b50, 0);
  trailer.writeUInt16LE(entries.length, 8);
  trailer.writeUInt16LE(entries.length, 10);
  trailer.writeUInt32LE(directorySize, 12);
  trailer.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, ...central, trailer]);
}

export async function validateBuilt(project, manifest) {
  return zip(await collect(project, manifest));
}

function inside(parent, child) {
  const rest = relative(parent, child);
  return (
    !rest ||
    (rest !== '..' &&
      !rest.startsWith(`..${sep}`) &&
      resolve(parent, rest) === child)
  );
}

export async function pack(project, manifest, output) {
  const destination = resolve(output);
  check(destination.endsWith('.awplugin'), 'Output must end with .awplugin');
  const assets = join(await realpath(project), 'dist');
  check(
    !inside(join(project, 'dist'), destination),
    'Output must be outside dist',
  );
  const archive = await validateBuilt(project, manifest);
  await mkdir(dirname(destination), {recursive: true});
  const directory = await realpath(dirname(destination));
  check(!inside(assets, directory), 'Output must be outside dist');
  const temporary = join(
    directory,
    `.${basename(destination)}.${randomUUID()}.tmp`,
  );
  try {
    await writeFile(temporary, archive, {flag: 'wx'});
    await rename(temporary, join(directory, basename(destination)));
  } finally {
    await rm(temporary, {force: true});
  }
  return archive.length;
}
