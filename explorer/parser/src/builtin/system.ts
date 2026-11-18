/**
 * The System Program's instructions. Data is a 4-byte little-endian variant
 * index followed by the variant's fields (bincode layout): u64 as 8 bytes
 * little-endian, an address as 32 raw bytes, a string as a u64 length and its
 * UTF-8 bytes. Variants, fields and account lists are those of
 * `SystemInstruction` in solana-system-interface 3.3.0.
 */

import type { ArgValue } from "../types.ts";
import { Reader, decodeWith, unknownInstruction } from "./reader.ts";
import type { BuiltinDecode } from "./reader.ts";

interface Variant {
  name: string;
  args: (reader: Reader) => Record<string, ArgValue>;
  accounts: (args: Record<string, ArgValue>, accountCount: number) => string[];
}

const none = (): Record<string, ArgValue> => ({});
const fixed = (...names: string[]) => (): string[] => names;

const VARIANTS = new Map<number, Variant>([
  [
    0,
    {
      name: "create_account",
      args: (r) => ({ lamports: r.u64("lamports"), space: r.u64("space"), owner: r.pubkey("owner") }),
      accounts: fixed("funding_account", "new_account"),
    },
  ],
  [
    1,
    {
      name: "assign",
      args: (r) => ({ owner: r.pubkey("owner") }),
      accounts: fixed("assigned_account"),
    },
  ],
  [
    2,
    {
      name: "transfer",
      args: (r) => ({ lamports: r.u64("lamports") }),
      accounts: fixed("funding_account", "recipient_account"),
    },
  ],
  [
    3,
    {
      name: "create_account_with_seed",
      args: (r) => ({
        base: r.pubkey("base"),
        seed: r.string64("seed"),
        lamports: r.u64("lamports"),
        space: r.u64("space"),
        owner: r.pubkey("owner"),
      }),
      accounts: (_args, count) =>
        count >= 3
          ? ["funding_account", "created_account", "base_account"]
          : ["funding_account", "created_account"],
    },
  ],
  [
    4,
    {
      name: "advance_nonce_account",
      args: none,
      accounts: fixed("nonce_account", "recent_blockhashes_sysvar", "nonce_authority"),
    },
  ],
  [
    5,
    {
      name: "withdraw_nonce_account",
      args: (r) => ({ lamports: r.u64("lamports") }),
      accounts: fixed(
        "nonce_account",
        "recipient_account",
        "recent_blockhashes_sysvar",
        "rent_sysvar",
        "nonce_authority",
      ),
    },
  ],
  [
    6,
    {
      name: "initialize_nonce_account",
      args: (r) => ({ authority: r.pubkey("authority") }),
      accounts: fixed("nonce_account", "recent_blockhashes_sysvar", "rent_sysvar"),
    },
  ],
  [
    7,
    {
      name: "authorize_nonce_account",
      args: (r) => ({ new_authority: r.pubkey("new authority") }),
      accounts: fixed("nonce_account", "nonce_authority"),
    },
  ],
  [
    8,
    {
      name: "allocate",
      args: (r) => ({ space: r.u64("space") }),
      accounts: fixed("new_account"),
    },
  ],
  [
    9,
    {
      name: "allocate_with_seed",
      args: (r) => ({
        base: r.pubkey("base"),
        seed: r.string64("seed"),
        space: r.u64("space"),
        owner: r.pubkey("owner"),
      }),
      accounts: fixed("allocated_account", "base_account"),
    },
  ],
  [
    10,
    {
      name: "assign_with_seed",
      args: (r) => ({
        base: r.pubkey("base"),
        seed: r.string64("seed"),
        owner: r.pubkey("owner"),
      }),
      accounts: fixed("assigned_account", "base_account"),
    },
  ],
  [
    11,
    {
      name: "transfer_with_seed",
      args: (r) => ({
        lamports: r.u64("lamports"),
        from_seed: r.string64("from seed"),
        from_owner: r.pubkey("from owner"),
      }),
      accounts: fixed("funding_account", "base_for_funding_account", "recipient_account"),
    },
  ],
  [12, { name: "upgrade_nonce_account", args: none, accounts: fixed("nonce_account") }],
  [
    13,
    {
      name: "create_account_allow_prefund",
      args: (r) => ({ lamports: r.u64("lamports"), space: r.u64("space"), owner: r.pubkey("owner") }),
      accounts: (args) =>
        args.lamports === "0" ? ["new_account"] : ["new_account", "funding_account"],
    },
  ],
]);

/** Decodes a System Program instruction. */
export function decodeSystem(data: Uint8Array, accountCount: number): BuiltinDecode {
  if (data.length < 4) {
    return unknownInstruction(`the data is ${data.length} bytes, shorter than the 4-byte tag.`);
  }
  const tag = new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(0, true);
  const variant = VARIANTS.get(tag);
  if (variant === undefined) {
    return unknownInstruction(`${tag} is not a System Program instruction.`);
  }
  return decodeWith(variant.name, data, 4, (reader) => {
    const args = variant.args(reader);
    return { args, accounts: variant.accounts(args, accountCount) };
  });
}
