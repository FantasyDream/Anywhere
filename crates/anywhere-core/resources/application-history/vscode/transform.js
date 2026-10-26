/** The SQLite reader supplies bounded rows; interpretation stays in this file. */
export function transform(input) {
  if (input.rows.length === 0) throw new Error('Recent history key missing');
  const document = JSON.parse(input.rows[0].value);
  if (!Array.isArray(document.entries))
    throw new Error('Unknown recent schema');
  const seen = new Set();
  const output = [];
  for (const entry of document.entries) {
    const uri = entry.folderUri ?? entry.fileUri ?? entry.workspace?.configPath;
    if (typeof uri !== 'string') throw new Error('Unsupported recent entry');
    if (seen.has(uri)) continue;
    seen.add(uri);
    const path = history.localPath(uri);
    let target;
    if (path) {
      target = {
        kind: entry.folderUri
          ? 'directory'
          : entry.workspace
            ? 'workspace'
            : 'file',
        path,
      };
    } else if (uri.startsWith('vscode-remote:')) {
      target = {kind: 'remote', uri};
    } else {
      throw new Error('Unsupported recent URI');
    }
    output.push({
      key: uri,
      title:
        entry.label ||
        (path || uri).split(/[\\/]/).filter(Boolean).pop() ||
        uri,
      target,
    });
  }
  return output;
}
