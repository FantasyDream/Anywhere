/** Extract recent projects from the host's XML tree; never read files here. */
export function transform(root) {
  if (root.name !== 'application')
    throw new Error('Unsupported recent-project document');
  const nodes = [];
  function walk(node) {
    nodes.push(node);
    for (const child of node.children) walk(child);
  }
  walk(root);
  const components = nodes.filter(
    node =>
      node.name === 'component' &&
      node.children.some(
        child =>
          child.name === 'option' &&
          ['additionalInfo', 'recentPaths'].includes(child.attributes.name),
      ),
  );
  if (components.length !== 1)
    throw new Error('Unknown or ambiguous recent-project schema');
  const component = components[0];
  const additional = component.children.find(
    node => node.name === 'option' && node.attributes.name === 'additionalInfo',
  );
  const paths = component.children.find(
    node => node.name === 'option' && node.attributes.name === 'recentPaths',
  );
  if (!additional && !paths) throw new Error('Unknown recent-project schema');
  const candidates = [];
  function collect(node, name) {
    if (node.name === name) candidates.push(node);
    for (const child of node.children) collect(child, name);
  }
  if (additional) collect(additional, 'entry');
  else collect(paths, 'option');
  const seen = new Set();
  const entries = [];
  for (const node of candidates) {
    const raw = node.attributes.key ?? node.attributes.value;
    if (!raw) continue;
    const path = history.localPath(raw.replace(/^\$USER_HOME\$/, '~'));
    if (!path || seen.has(path)) continue;
    seen.add(path);
    const metadata = [];
    function options(child) {
      if (child.name === 'option') metadata.push(child.attributes);
      for (const next of child.children) options(next);
    }
    options(node);
    const title = metadata.find(a => a.name === 'displayName')?.value;
    const time = metadata.find(a => a.name === 'projectOpenTimestamp')?.value;
    entries.push({
      key: path,
      title: title || path.split(/[\\/]/).filter(Boolean).pop() || path,
      target: {
        kind: /\.(sln|slnx|slnf)$/i.test(path) ? 'file' : 'directory',
        path,
      },
      last_used_at: time ? Number(time) : null,
    });
  }
  return entries;
}
