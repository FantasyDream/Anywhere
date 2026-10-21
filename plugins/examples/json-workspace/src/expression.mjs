export class QueryError extends Error {}
export const MAX_QUERY_LENGTH = 1024;

/** The editor stores only the suffix; accept a full expression pasted from elsewhere. */
export function stripRoot(value) {
  return value.replace(/^\s*(?:this|\$)(?=$|[^\w$])/, '');
}

export function formula(value, mode = 'jsonpath') {
  let suffix = stripRoot(value).trim();
  if (suffix.length > MAX_QUERY_LENGTH) throw new QueryError('Formula exceeds 1,024 characters');
  if (/^[\p{L}_$]/u.test(suffix)) suffix = '.' + suffix;
  return (mode === 'javascript' ? 'this' : '$') + suffix;
}
