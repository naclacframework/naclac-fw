/**
 * Names the accounts of one instruction from a declared list (an IDL's, or a
 * built-in program's), shared by naclac and built-in decoding so both behave
 * the same way.
 */

import type { FlatInstruction } from "./flatten.ts";
import { ParseError } from "./errors.ts";
import type { AccountKey, DecodedAccount } from "./types.ts";

export interface DeclaredAccount {
  name: string;
  optional?: boolean;
}

/**
 * Returns the instruction's accounts with the declared names by position and
 * the transaction's signer and writable flags. Accounts past the declared list
 * are named `<extraPrefix>1`, `<extraPrefix>2`, ...; declared accounts the
 * instruction did not supply are returned in `missing`. An optional account
 * filled with the program's own address is marked not provided.
 */
export function nameAccounts(
  declared: readonly DeclaredAccount[],
  instruction: FlatInstruction,
  keys: AccountKey[],
  extraPrefix: string,
): { accounts: DecodedAccount[]; missing: string[] } {
  const accounts = instruction.accountIndexes.map((keyIndex, position): DecodedAccount => {
    const key = keys[keyIndex];
    if (key === undefined) {
      throw new ParseError(`account index ${keyIndex} is out of range.`, "message.instructions");
    }
    const account = declared[position];
    return {
      name: account === undefined ? `${extraPrefix}${position - declared.length + 1}` : account.name,
      pubkey: key.address,
      isSigner: key.signer,
      isWritable: key.writable,
      provided: !(account?.optional === true && key.address === instruction.programId),
    };
  });
  return {
    accounts,
    missing: declared.slice(instruction.accountIndexes.length).map((account) => account.name),
  };
}
