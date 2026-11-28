import {lstat, readFile} from 'node:fs/promises';
import {join} from 'node:path';

export const maxManifestBytes = 128 * 1024;
const schemaUrl = new URL(
  '../schemas/web-plugin-manifest-editor.schema.json',
  import.meta.url,
);
const repositorySchema = new URL(
  '../../../resources/web-plugin-manifest-editor.schema.json',
  import.meta.url,
);

export function check(condition, message) {
  if (!condition) throw new Error(message);
}

// Scan the syntax before JSON.parse can collapse a repeated property. Decode
// string keys so an escaped spelling cannot bypass the uniqueness check.
export function parseUniqueJson(source) {
  const token =
    /[ \t\r\n]*("(?:[^"\\]|\\["\\/bfnrt]|\\u[0-9a-fA-F]{4})*"|[{}[\]:,]|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null)/y;
  function next() {
    return token.exec(source)?.[1];
  }
  function value(first, depth) {
    check(depth < 128, 'JSON nesting exceeds the limit');
    if (first === '{') {
      const keys = new Set();
      let key = next();
      if (key === '}') return;
      for (;;) {
        check(key?.startsWith('"'), 'Expected a JSON property name');
        const name = JSON.parse(key);
        check(!keys.has(name), `Duplicate JSON property: ${name}`);
        keys.add(name);
        check(next() === ':', 'Expected a JSON property value');
        value(next(), depth + 1);
        const separator = next();
        if (separator === '}') return;
        check(separator === ',', 'Expected a JSON object separator');
        key = next();
      }
    }
    if (first === '[') {
      let item = next();
      if (item === ']') return;
      for (;;) {
        value(item, depth + 1);
        const separator = next();
        if (separator === ']') return;
        check(separator === ',', 'Expected a JSON array separator');
        item = next();
      }
    }
    check(first && !'];}:,'.includes(first), 'Expected a JSON value');
  }
  value(next(), 0);
  return JSON.parse(source);
}

const isObject = value =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

// Only the JSON Schema keywords emitted for the Manifest DTO are needed here.
function schemaIssue(shape, value, at, root) {
  if (shape.$ref)
    return schemaIssue(
      root.$defs[shape.$ref.slice('#/$defs/'.length)],
      value,
      at,
      root,
    );
  const options = shape.anyOf ?? shape.oneOf;
  if (options && !options.some(option => !schemaIssue(option, value, at, root)))
    return `${at} has an invalid shape`;
  const types = [shape.type].flat();
  const matchingType = type => {
    switch (type) {
      case 'object':
        return isObject(value);
      case 'array':
        return Array.isArray(value);
      case 'integer':
        return Number.isSafeInteger(value);
      case 'null':
        return value === null;
      case undefined:
        return true;
      default:
        return typeof value === type;
    }
  };
  if (!types.some(matchingType)) return `${at} has an invalid type`;
  if (Object.hasOwn(shape, 'const') && value !== shape.const)
    return `${at} must be ${JSON.stringify(shape.const)}`;
  if (shape.enum && !shape.enum.includes(value))
    return `${at} is not a supported value`;
  if (Array.isArray(value)) {
    if (value.length > (shape.maxItems ?? Infinity))
      return `${at} has too many entries`;
    for (const [index, item] of value.entries()) {
      const error = schemaIssue(
        shape.items ?? {},
        item,
        `${at}[${index}]`,
        root,
      );
      if (error) return error;
    }
  } else if (isObject(value)) {
    for (const name of shape.required ?? [])
      if (!Object.hasOwn(value, name)) return `${at}.${name} is required`;
    for (const [name, item] of Object.entries(value)) {
      const property =
        shape.properties && Object.hasOwn(shape.properties, name)
          ? shape.properties[name]
          : shape.additionalProperties;
      if (property === false) return `${at}.${name} is unknown`;
      if (property) {
        const error = schemaIssue(property, item, `${at}.${name}`, root);
        if (error) return error;
      }
    }
  }
  return undefined;
}

const segment = value => /^[a-z][a-z0-9_-]{0,63}$/.test(value);
const namespace = value =>
  value.length <= 160 && value.includes('.') && value.split('.').every(segment);
const exportName = value => /^[$a-zA-Z_][$a-zA-Z_0-9]{0,63}$/.test(value);
const label = (value, max) =>
  value.trim() !== '' &&
  Buffer.byteLength(value) <= max &&
  !/\p{Cc}|\p{Cs}/u.test(value);
const unique = values => new Set(values).size === values.length;
const knownContext = new Set([
  'core.text',
  'core.url',
  'core.file',
  'core.directory',
  'core.file_selection',
]);
const semver =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

export function checkAssetPath(path) {
  const parts = path.split('/');
  check(
    Buffer.byteLength(path) <= 1024 && parts.length <= 16,
    `Invalid asset path: ${path}`,
  );
  for (const part of parts) {
    check(
      part &&
        Buffer.byteLength(part) <= 255 &&
        !/[. ]$/.test(part) &&
        !/[\\:<>"|?*%#\p{Cc}\p{Cs}]/u.test(part),
      `Non-portable asset path: ${path}`,
    );
    check(
      !/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|(?:COM|LPT)[1-9¹²³])$/iu.test(
        part.split('.')[0],
      ),
      `Reserved asset path: ${path}`,
    );
  }
}

function validateSemantics(manifest, schema, warn) {
  const {id, name, version, platforms, entry, background, icon, permissions} =
    manifest;
  const commands = manifest.commands ?? [];
  const resolvers = background?.resolvers ?? [];
  check(
    namespace(id) &&
      !['core', 'anywhere', 'first-party', 'web', 'plugin'].includes(
        id.split('.')[0],
      ),
    'Invalid or reserved plugin id',
  );
  check(label(name, 200), 'Invalid plugin name');
  const parsed = semver.exec(version);
  check(
    version.length <= 128 &&
      parsed &&
      parsed.slice(1, 4).every(part => BigInt(part) <= 0xffffffffffffffffn) &&
      !parsed[4]?.split('.').some(part => /^0\d+$/.test(part)),
    'Invalid SemVer package version',
  );
  check(
    platforms.length && unique(platforms),
    'Expected unique supported platforms',
  );
  if (entry) {
    checkAssetPath(entry);
    check(entry.endsWith('.html'), 'Web entry must be local HTML');
  }
  if (background) {
    checkAssetPath(background.entry);
    check(
      /\.m?js$/.test(background.entry),
      'Background entry must be compiled JavaScript',
    );
  }
  if (icon !== null && icon !== undefined) {
    checkAssetPath(icon);
    check(icon.endsWith('.png'), 'Package icon must be PNG');
  }
  const ownedPrefix = `plugin.${id}.`;
  const permissionNames = permissions.map(permission => permission.name);
  check(unique(permissionNames), 'Duplicate permission');
  const contextTypes =
    permissions.find(permission => permission.name === 'context.read')?.types ??
    [];
  const types = (values, min, max) =>
    values.length >= min &&
    values.length <= max &&
    unique(values) &&
    values.every(namespace);
  check(
    !permissionNames.includes('context.read') ||
      (types(contextTypes, 1, 16) &&
        contextTypes.every(
          type => knownContext.has(type) || type.startsWith(ownedPrefix),
        )),
    'Invalid Context projection types',
  );
  const origins = permissions.find(
    permission => permission.name === 'url.open',
  )?.origins;
  if (origins && !(origins.length === 1 && origins[0] === '*')) {
    check(
      origins.length >= 1 && origins.length <= 16,
      'Expected up to 16 URL origins',
    );
    const canonical = origins.map(origin => {
      check(
        /^https?:\/\/[^/?#@*\\\s]+$/i.test(origin) && !/\p{Cc}/u.test(origin),
        'URL grant must be an exact HTTP(S) origin',
      );
      return new URL(origin).origin;
    });
    check(unique(canonical), 'Duplicate URL origin');
  }
  if (
    permissionNames.some(
      permission =>
        permission === 'resources.read' || permission === 'preview.open',
    )
  ) {
    check(
      contextTypes.includes('core.file') &&
        commands.some(command => command.accepts?.includes('core.file')),
      'File grants require context.read for core.file and a command that accepts it',
    );
  }
  check(
    resolvers.length <= 16 && unique(resolvers.map(resolver => resolver.id)),
    'Too many or duplicate resolvers',
  );
  for (const resolver of resolvers) {
    check(
      segment(resolver.id) &&
        exportName(resolver.entrypoint) &&
        types(resolver.accepts, 1, 8) &&
        types(resolver.produces, 1, 8) &&
        resolver.accepts.every(type => contextTypes.includes(type)) &&
        resolver.produces.every(type => type.startsWith(ownedPrefix)),
      `Invalid resolver: ${resolver.id}`,
    );
  }
  check(
    commands.length <= 32 &&
      (commands.length || resolvers.length) &&
      unique(commands.map(command => command.id)),
    'Expected 1..32 unique commands or at least one resolver',
  );
  for (const command of commands) {
    const {
      id,
      entrypoint,
      runtime,
      title,
      localizedTitles = {},
      subtitle,
      icon,
      accepts = [],
      keywords = [],
      inputMatchers = [],
    } = command;
    check(segment(id), `Invalid command id: ${id}`);
    check(
      runtime === 'background'
        ? background && exportName(entrypoint)
        : entry && segment(entrypoint),
      `Missing or invalid command entrypoint: ${id}`,
    );
    check(
      label(title, 200) &&
        Object.entries(localizedTitles).every(
          ([language, text]) =>
            language.length <= 35 &&
            /^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(language) &&
            label(text, 200),
        ),
      `Invalid command title: ${id}`,
    );
    check(
      subtitle === null || subtitle === undefined || label(subtitle, 200),
      `Invalid command subtitle: ${id}`,
    );
    check(
      icon === null || icon === undefined || label(icon, 64),
      `Invalid command icon: ${id}`,
    );
    if (icon && !schema['x-anywhere-commandIcons'].includes(icon))
      warn(`Command ${id} uses unknown icon ${icon}. Anywhere displays code.`);
    check(
      types(accepts, command.requireContext ? 1 : 0, 16),
      `Invalid command Context types: ${id}`,
    );
    check(
      keywords.length <= 16 &&
        unique(keywords) &&
        keywords.every(word => label(word, 100)),
      `Invalid command keywords: ${id}`,
    );
    check(
      unique(inputMatchers) &&
        (!command.requireInputMatch || inputMatchers.length) &&
        (!inputMatchers.length ||
          (contextTypes.includes('core.text') &&
            accepts.includes('core.text'))),
      `Invalid input matchers: ${id}`,
    );
  }
}

export async function readManifest(project, warn = () => {}) {
  const file = join(project, 'manifest.json');
  const stat = await lstat(file);
  check(
    stat.isFile() && stat.size <= maxManifestBytes,
    'Manifest must be a regular file up to 128 KiB',
  );
  const bytes = await readFile(file);
  check(bytes.length <= maxManifestBytes, 'Manifest exceeds 128 KiB');
  const manifest = parseUniqueJson(
    new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(bytes),
  );
  const schema = JSON.parse(
    await readFile(schemaUrl, 'utf8').catch(error => {
      if (error.code !== 'ENOENT') throw error;
      return readFile(repositorySchema, 'utf8');
    }),
  );
  if (manifest?.apiVersion === '1')
    throw new Error('Rebuild the package with the Anywhere API 2 SDK');
  const error = schemaIssue(schema, manifest, 'manifest', schema);
  check(!error, error);
  validateSemantics(manifest, schema, warn);
  return manifest;
}
