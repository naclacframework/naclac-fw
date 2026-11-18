/**
 * Builds a transaction's full account list: the message's static keys, then the
 * address-lookup-table writable keys, then the readonly ones. This is the order
 * instruction account indexes and `meta` balances use.
 */

import { ParseError } from "./errors.ts";
import type { AccountKey, RpcLoadedAddresses, RpcMessage } from "./types.ts";

function checkHeader(message: RpcMessage): void {
  const staticCount = message.accountKeys.length;
  const { numRequiredSignatures, numReadonlySignedAccounts, numReadonlyUnsignedAccounts } =
    message.header;
  const counts = [numRequiredSignatures, numReadonlySignedAccounts, numReadonlyUnsignedAccounts];
  if (!counts.every((count) => Number.isInteger(count) && count >= 0)) {
    throw new ParseError("message.header counts are not non-negative integers.", "message.header");
  }
  if (numRequiredSignatures < 1 || numRequiredSignatures > staticCount) {
    throw new ParseError(
      `header.numRequiredSignatures is ${numRequiredSignatures} for ${staticCount} account keys.`,
      "message.header.numRequiredSignatures",
    );
  }
  if (numReadonlySignedAccounts >= numRequiredSignatures) {
    throw new ParseError(
      `header.numReadonlySignedAccounts (${numReadonlySignedAccounts}) leaves no writable signer.`,
      "message.header.numReadonlySignedAccounts",
    );
  }
  if (numReadonlyUnsignedAccounts > staticCount - numRequiredSignatures) {
    throw new ParseError(
      `header.numReadonlyUnsignedAccounts (${numReadonlyUnsignedAccounts}) exceeds the unsigned keys.`,
      "message.header.numReadonlyUnsignedAccounts",
    );
  }
}

function loadedLists(
  message: RpcMessage,
  loaded: RpcLoadedAddresses | null | undefined,
): { writable: string[]; readonly: string[] } {
  if (loaded === null || loaded === undefined) {
    if ((message.addressTableLookups?.length ?? 0) > 0) {
      throw new ParseError(
        "meta.loadedAddresses is missing for a message that uses address lookup tables.",
        "meta.loadedAddresses",
      );
    }
    return { writable: [], readonly: [] };
  }
  if (!Array.isArray(loaded.writable) || !Array.isArray(loaded.readonly)) {
    throw new ParseError(
      "meta.loadedAddresses must hold a writable list and a readonly list.",
      "meta.loadedAddresses",
    );
  }
  return loaded;
}

/**
 * Returns every account key of the transaction with its signer and writable
 * flags. Throws a ParseError when the header does not fit the key list, or
 * when lookup-table keys are expected but the node did not return them.
 */
export function resolveAccountKeys(
  message: RpcMessage,
  loaded: RpcLoadedAddresses | null | undefined,
): AccountKey[] {
  checkHeader(message);
  const staticKeys = message.accountKeys;
  const staticCount = staticKeys.length;
  const { numRequiredSignatures, numReadonlySignedAccounts, numReadonlyUnsignedAccounts } =
    message.header;

  const keys: AccountKey[] = staticKeys.map((address, index) => ({
    index,
    address,
    signer: index < numRequiredSignatures,
    writable:
      index < numRequiredSignatures - numReadonlySignedAccounts ||
      (index >= numRequiredSignatures && index < staticCount - numReadonlyUnsignedAccounts),
    source: "static",
  }));

  const lists = loadedLists(message, loaded);
  for (const address of lists.writable) {
    keys.push({
      index: keys.length,
      address,
      signer: false,
      writable: true,
      source: "lookup-writable",
    });
  }
  for (const address of lists.readonly) {
    keys.push({
      index: keys.length,
      address,
      signer: false,
      writable: false,
      source: "lookup-readonly",
    });
  }
  return keys;
}
