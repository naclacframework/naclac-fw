/**
 * The well-known Solana programs the parser decodes without an IDL: System,
 * Token, Token-2022, Associated Token Account, Compute Budget and Memo. These
 * programs always decode with their own decoder; an IDL supplied for one of
 * their addresses is not used.
 */

import { decodeBase58 } from "../base58.ts";
import type { FlatInstruction } from "../flatten.ts";
import { nameAccounts } from "../accounts.ts";
import type { InstructionDecoding } from "../naclac.ts";
import type { AccountKey } from "../types.ts";
import { decodeAssociatedToken } from "./ata.ts";
import { decodeComputeBudget } from "./compute-budget.ts";
import { decodeMemo } from "./memo.ts";
import type { BuiltinDecoder } from "./reader.ts";
import { decodeSystem } from "./system.ts";
import { decodeToken } from "./token.ts";

interface Builtin {
  programName: string;
  decode: BuiltinDecoder;
  extraAccountPrefix: string;
}

const MEMO: Builtin = { programName: "memo", decode: decodeMemo, extraAccountPrefix: "signer" };

export const BUILTIN_PROGRAMS: ReadonlyMap<string, Builtin> = new Map([
  [
    "11111111111111111111111111111111",
    { programName: "system", decode: decodeSystem, extraAccountPrefix: "remainingAccount" },
  ],
  [
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    {
      programName: "token",
      decode: (data) => decodeToken("token", data),
      extraAccountPrefix: "remainingAccount",
    },
  ],
  [
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    {
      programName: "token_2022",
      decode: (data) => decodeToken("token-2022", data),
      extraAccountPrefix: "remainingAccount",
    },
  ],
  [
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
    {
      programName: "associated_token_account",
      decode: decodeAssociatedToken,
      extraAccountPrefix: "remainingAccount",
    },
  ],
  [
    "ComputeBudget111111111111111111111111111111",
    {
      programName: "compute_budget",
      decode: decodeComputeBudget,
      extraAccountPrefix: "remainingAccount",
    },
  ],
  ["Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo", MEMO],
  ["MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr", MEMO],
]);

/**
 * Decodes `instruction` when its program is a built-in one, or returns null
 * when it is not.
 */
export function decodeBuiltin(
  instruction: FlatInstruction,
  keys: AccountKey[],
): InstructionDecoding | null {
  const builtin = BUILTIN_PROGRAMS.get(instruction.programId);
  if (builtin === undefined) return null;

  const data = decodeBase58(instruction.rawData, "instruction data");
  const result = builtin.decode(data, instruction.accountIndexes.length);
  if (!result.ok) {
    return {
      programName: builtin.programName,
      instructionName: result.name,
      decoded: null,
      decodeStatus: result.status,
      decodeReason: result.reason,
    };
  }
  const { accounts, missing } = nameAccounts(
    result.accounts.map((name) => ({ name })),
    instruction,
    keys,
    builtin.extraAccountPrefix,
  );
  return {
    programName: builtin.programName,
    instructionName: result.name,
    decoded: {
      args: result.args,
      accounts,
      missingAccounts: missing,
      trailingBytes: result.trailingBytes,
    },
    decodeStatus: "decoded",
    decodeReason: null,
  };
}
