/**
 * The Memo Program: the whole instruction data is a UTF-8 string, and every
 * account is a signer (zero or more), as described by spl-memo and
 * spl-memo-interface.
 */

import { decodeWith, utf8 } from "./reader.ts";
import type { BuiltinDecode } from "./reader.ts";

/** Decodes a Memo Program instruction. */
export function decodeMemo(data: Uint8Array): BuiltinDecode {
  return decodeWith("memo", data, 0, (reader) => ({
    args: { memo: utf8(reader.rest(), "the memo") },
    accounts: [],
  }));
}
