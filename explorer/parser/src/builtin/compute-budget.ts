/**
 * The Compute Budget Program's instructions: a 1-byte tag followed by one
 * little-endian value, as in `ComputeBudgetInstruction` of
 * solana-compute-budget-interface 3.1.0. The program takes no accounts.
 */

import type { ArgValue } from "../types.ts";
import { Reader, decodeWith, unknownInstruction } from "./reader.ts";
import type { BuiltinDecode } from "./reader.ts";

const VARIANTS = new Map<number, { name: string; args: (reader: Reader) => Record<string, ArgValue> }>([
  [0, { name: "unused", args: () => ({}) }],
  [1, { name: "request_heap_frame", args: (r) => ({ bytes: r.u32("bytes") }) }],
  [2, { name: "set_compute_unit_limit", args: (r) => ({ units: r.u32("units") }) }],
  [3, { name: "set_compute_unit_price", args: (r) => ({ micro_lamports: r.u64("micro-lamports") }) }],
  [4, { name: "set_loaded_accounts_data_size_limit", args: (r) => ({ bytes: r.u32("bytes") }) }],
]);

/** Decodes a Compute Budget Program instruction. */
export function decodeComputeBudget(data: Uint8Array): BuiltinDecode {
  const tag = data[0];
  if (tag === undefined) return unknownInstruction("the data is empty.");
  const variant = VARIANTS.get(tag);
  if (variant === undefined) {
    return unknownInstruction(`${tag} is not a Compute Budget Program instruction.`);
  }
  return decodeWith(variant.name, data, 1, (reader) => ({ args: variant.args(reader), accounts: [] }));
}
