/**
 * The Associated Token Account Program's instructions: a 1-byte tag (0 create,
 * 1 create idempotent, 2 recover nested) as in
 * spl-associated-token-account-interface 2.0.0; empty data means create, as the
 * program's processor treats it (spl-associated-token-account 8.0.0).
 */

import { decodeWith, unknownInstruction } from "./reader.ts";
import type { BuiltinDecode } from "./reader.ts";

const CREATE_ACCOUNTS = [
  "funding_account",
  "associated_token_account",
  "wallet_address",
  "token_mint",
  "system_program",
  "token_program",
];

const VARIANTS = new Map<number, { name: string; accounts: string[] }>([
  [0, { name: "create", accounts: CREATE_ACCOUNTS }],
  [1, { name: "create_idempotent", accounts: CREATE_ACCOUNTS }],
  [
    2,
    {
      name: "recover_nested",
      accounts: [
        "nested_associated_token_account",
        "nested_token_mint",
        "destination_associated_token_account",
        "owner_associated_token_account",
        "owner_token_mint",
        "wallet_address",
        "token_program",
      ],
    },
  ],
]);

/** Decodes an Associated Token Account Program instruction. */
export function decodeAssociatedToken(data: Uint8Array): BuiltinDecode {
  const tag = data.length === 0 ? 0 : (data[0] as number);
  const variant = VARIANTS.get(tag);
  if (variant === undefined) {
    return unknownInstruction(`${tag} is not an Associated Token Account Program instruction.`);
  }
  return decodeWith(variant.name, data, data.length === 0 ? 0 : 1, () => ({
    args: {},
    accounts: variant.accounts,
  }));
}
