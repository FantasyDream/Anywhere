import type {Calls} from './contract.js';
import {limits} from './contract.js';

function utf8Size(code: number) {
  return code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
}

/** No TextEncoder or browser globals: this also runs inside finite QuickJS jobs.
 * Report validity separately so each adapter retains its validation precedence.
 */
export function inspectText(text: string) {
  let bytes = 0,
    wellFormed = true;
  for (let offset = 0; offset < text.length;) {
    const code = text.codePointAt(offset)!;
    if (code >= 0xd800 && code <= 0xdfff) wellFormed = false;
    bytes += utf8Size(code);
    offset += code > 0xffff ? 2 : 1;
  }
  return {bytes, wellFormed};
}

/** Slice validated text on scalar boundaries; preserve BOMs and avoid a full byte copy. */
function* chunks(text: string) {
  let start = 0,
    offset = 0,
    bytes = 0;
  while (offset < text.length) {
    const code = text.codePointAt(offset)!;
    const size = utf8Size(code);
    if (bytes + size > limits.chunkBytes) {
      yield text.slice(start, offset);
      start = offset;
      bytes = 0;
    }
    bytes += size;
    offset += code > 0xffff ? 2 : 1;
  }
  if (offset > start) yield text.slice(start, offset);
}

interface TextUpload {
  begin(bytes: number): Promise<{ticket: string}>;
  chunk(params: Calls['text.chunk']): Promise<unknown>;
  commit(ticket: string): Promise<unknown>;
  cancel(ticket: string): void | Promise<unknown>;
}

/** Upload once, in order. The adapter owns cancellation lifetime and scheduling. */
export async function uploadText(
  text: string,
  bytes: number,
  transport: TextUpload,
) {
  const {ticket} = await transport.begin(bytes);
  try {
    let sequence = 0;
    for (const chunk of chunks(text))
      await transport.chunk({ticket, sequence: sequence++, text: chunk});
    await transport.commit(ticket);
  } catch (error) {
    try {
      const cleanup = transport.cancel(ticket);
      if (cleanup) await cleanup;
    } catch {
      // Cleanup must not replace the failed operation or retry its side effect.
    }
    throw error;
  }
}
