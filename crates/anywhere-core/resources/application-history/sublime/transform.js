/** Read only recorded session history; buffers and unsaved document text are ignored. */
export function transform(input) {
  const groups = [
    [input.folder_history, 'directory'],
    [input.file_history, 'file'],
    [input.workspaces?.recent_workspaces, 'file'],
  ];
  if (!groups.some(([values]) => Array.isArray(values))) {
    throw new Error('Unknown Sublime session schema');
  }
  const seen = new Set();
  const items = [];
  for (const [values, kind] of groups) {
    if (values === undefined) continue;
    if (!Array.isArray(values)) throw new Error('Invalid session history');
    for (const value of values) {
      if (typeof value !== 'string') throw new Error('Invalid session path');
      const path = history.localPath(value);
      if (!path) throw new Error('Unsupported session path');
      if (seen.has(path)) continue;
      seen.add(path);
      items.push({
        key: path,
        title: path.split(/[\\/]/).filter(Boolean).pop() || path,
        target: {kind, path},
      });
    }
  }
  return items;
}
