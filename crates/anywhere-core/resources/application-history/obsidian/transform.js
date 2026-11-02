/** Global metadata lists vaults; never enumerate vault contents or infer file access times. */
export function transform(input) {
  if (
    !input.vaults ||
    typeof input.vaults !== 'object' ||
    Array.isArray(input.vaults)
  ) {
    throw new Error('Unknown Obsidian vault schema');
  }
  return Object.entries(input.vaults).map(([vaultId, vault]) => {
    if (typeof vault.path !== 'string') throw new Error('Invalid vault path');
    const path = history.localPath(vault.path);
    if (!path) throw new Error('Unsupported vault path');
    return {
      key: vaultId,
      title: path.split(/[\\/]/).filter(Boolean).pop() || path,
      target: {kind: 'obsidian_vault', path, vault_id: vaultId},
      last_used_at: vault.ts ?? null,
    };
  });
}
