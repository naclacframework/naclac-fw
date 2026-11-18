/**
 * Token balance changes: each token account's balance before and after the
 * transaction, from the node's `preTokenBalances` and `postTokenBalances`.
 * These are the source of truth for token movement; transfers extracted from
 * instructions are checked against them.
 */

import { ParseError } from "./errors.ts";
import type { AccountKey, RpcMeta, RpcTokenBalance, TokenBalanceChange } from "./types.ts";

function indexEntries(
  list: RpcTokenBalance[],
  keys: AccountKey[],
  field: string,
): Map<number, RpcTokenBalance> {
  const byIndex = new Map<number, RpcTokenBalance>();
  list.forEach((entry, position) => {
    const where = `${field}[${position}]`;
    if (!Number.isInteger(entry.accountIndex) || keys[entry.accountIndex] === undefined) {
      throw new ParseError(`${where}.accountIndex is not an account of the transaction.`, where);
    }
    if (typeof entry.mint !== "string" || entry.mint === "") {
      throw new ParseError(`${where}.mint is missing.`, where);
    }
    const amount = entry.uiTokenAmount?.amount;
    if (typeof amount !== "string" || !/^\d+$/.test(amount)) {
      throw new ParseError(`${where}.uiTokenAmount.amount is not a decimal integer string.`, where);
    }
    const decimals = entry.uiTokenAmount.decimals;
    if (!Number.isInteger(decimals) || decimals < 0 || decimals > 255) {
      throw new ParseError(`${where}.uiTokenAmount.decimals is not a byte.`, where);
    }
    if (byIndex.has(entry.accountIndex)) {
      throw new ParseError(`${field} lists account ${entry.accountIndex} twice.`, field);
    }
    byIndex.set(entry.accountIndex, entry);
  });
  return byIndex;
}

/**
 * Returns the balance change of every token account the node listed, ordered
 * by account index, or null when the node returned no token balances at all.
 * Throws a ParseError when only one of the two lists is present, an entry is
 * malformed, or an account changed mint.
 */
export function tokenBalanceChanges(meta: RpcMeta, keys: AccountKey[]): TokenBalanceChange[] | null {
  const pre = meta.preTokenBalances ?? null;
  const post = meta.postTokenBalances ?? null;
  if (pre === null && post === null) return null;
  if (pre === null || post === null) {
    throw new ParseError(
      "only one of meta.preTokenBalances and meta.postTokenBalances is present.",
      "meta.preTokenBalances",
    );
  }
  const before = indexEntries(pre, keys, "meta.preTokenBalances");
  const after = indexEntries(post, keys, "meta.postTokenBalances");

  return [...new Set([...before.keys(), ...after.keys()])]
    .sort((a, b) => a - b)
    .map((index): TokenBalanceChange => {
      const was = before.get(index);
      const now = after.get(index);
      const entry = (now ?? was) as RpcTokenBalance;
      if (was !== undefined && now !== undefined && was.mint !== now.mint) {
        throw new ParseError(`token account ${index} changed mint during the transaction.`, "meta.postTokenBalances");
      }
      const address = (keys[index] as AccountKey).address;
      const preAmount = was === undefined ? null : was.uiTokenAmount.amount;
      const postAmount = now === undefined ? null : now.uiTokenAmount.amount;
      return {
        index,
        address,
        mint: entry.mint,
        owner: entry.owner ?? null,
        programId: entry.programId ?? null,
        decimals: entry.uiTokenAmount.decimals,
        pre: preAmount,
        post: postAmount,
        change: (BigInt(postAmount ?? "0") - BigInt(preAmount ?? "0")).toString(),
      };
    });
}
