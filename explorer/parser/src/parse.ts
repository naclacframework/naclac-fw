/**
 * Entry point of the pure parse step: takes the raw `getTransaction` result,
 * and the IDLs of the naclac programs it may call, and returns the parsed
 * transaction. No network. Programs without an IDL keep their raw instruction
 * data and are marked `no-idl`.
 */

import { resolveAccountKeys } from "./account-keys.ts";
import { BUILTIN_PROGRAMS, decodeBuiltin } from "./builtin/index.ts";
import { ParseError } from "./errors.ts";
import { flattenInstructions } from "./flatten.ts";
import type { NaclacIdl } from "./idl.ts";
import { tokenBalanceChanges } from "./token-balances.ts";
import { extractTransfers } from "./transfers.ts";
import { walkLogs } from "./logs.ts";
import { decodeFailure, decodeInstruction, decodeLogEvents } from "./naclac.ts";
import type {
  AccountKey,
  BalanceChange,
  ParsedEvent,
  ParsedInstruction,
  ParsedTransaction,
  ProgramIdlRef,
  RpcMeta,
  RpcTransactionResult,
  TransactionVersion,
  UndecodedLogData,
} from "./types.ts";

export const PARSER_VERSION = 5;

const LOG_TRUNCATED = "Log truncated";

export interface ParseOptions {
  idls?: ReadonlyMap<string, NaclacIdl>;
}

function checkVersion(version: unknown): TransactionVersion {
  if (version === "legacy" || version === 0 || version === 1) return version;
  if (version === undefined) {
    throw new ParseError(
      "transaction version is missing; fetch with maxSupportedTransactionVersion: 1.",
      "version",
    );
  }
  throw new ParseError(`transaction version ${JSON.stringify(version)} is not supported.`, "version");
}

function lamports(value: unknown, field: string): bigint {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new ParseError(`${field} is not a safe non-negative integer.`, field);
  }
  return BigInt(value);
}

function balanceChanges(meta: RpcMeta, keys: AccountKey[]): BalanceChange[] {
  const { preBalances, postBalances } = meta;
  if (preBalances.length !== keys.length || postBalances.length !== keys.length) {
    throw new ParseError(
      `meta has ${preBalances.length} pre and ${postBalances.length} post balances for ${keys.length} account keys.`,
      "meta.preBalances",
    );
  }
  return keys.map((key, index) => {
    const pre = lamports(preBalances[index], `meta.preBalances[${index}]`);
    const post = lamports(postBalances[index], `meta.postBalances[${index}]`);
    return {
      index,
      address: key.address,
      pre: pre.toString(),
      post: post.toString(),
      change: (post - pre).toString(),
    };
  });
}

/**
 * Parses one raw `getTransaction` result. Throws a ParseError naming the field
 * when the result is incomplete or inconsistent; it never guesses a missing
 * value.
 */
export function parseRawTransaction(
  result: RpcTransactionResult,
  options: ParseOptions = {},
): ParsedTransaction {
  const idls = new Map(
    [...(options.idls ?? new Map<string, NaclacIdl>())].filter(
      ([programId]) => !BUILTIN_PROGRAMS.has(programId),
    ),
  );
  const meta = result.meta;
  if (meta === null || meta === undefined) {
    throw new ParseError("meta is missing; the node returned no execution metadata.", "meta");
  }
  if (meta.err === undefined) {
    throw new ParseError("meta.err is missing.", "meta.err");
  }
  const { message, signatures } = result.transaction;
  const signature = signatures[0];
  if (signature === undefined) {
    throw new ParseError("the transaction has no signatures.", "transaction.signatures");
  }
  const version = checkVersion(result.version);
  const keys = resolveAccountKeys(message, meta.loadedAddresses);
  const feePayer = keys[0];
  if (feePayer === undefined) {
    throw new ParseError("the transaction has no account keys.", "message.accountKeys");
  }
  const flat = flattenInstructions(message.instructions, meta.innerInstructions ?? [], keys);
  const logs = meta.logMessages ?? null;
  const walked = logs === null ? null : walkLogs(logs);

  const instructions: ParsedInstruction[] = flat.map((entry) => ({
    instructionIndex: entry.instructionIndex,
    innerInstructionIndex: entry.innerInstructionIndex,
    stackHeight: entry.stackHeight,
    programId: entry.programId,
    rawAccounts: entry.rawAccounts,
    rawData: entry.rawData,
    ...(decodeBuiltin(entry, keys) ?? decodeInstruction(idls.get(entry.programId), entry, keys)),
  }));

  let events: ParsedEvent[] = [];
  let undecodedLogData: UndecodedLogData[] = [];
  if (walked !== null) {
    const decoded = decodeLogEvents(walked, flat, idls);
    events = decoded.events;
    undecodedLogData = decoded.undecoded;
  }

  const idlRefs: ProgramIdlRef[] = [...new Set(flat.map((entry) => entry.programId))].flatMap(
    (programId) => {
      const idl = idls.get(programId);
      return idl === undefined
        ? []
        : [{ programId, name: idl.name, serialization: idl.serialization }];
    },
  );

  const balances = balanceChanges(meta, keys);
  const tokenChanges = tokenBalanceChanges(meta, keys);
  const transfers = extractTransfers({
    succeeded: meta.err === null,
    instructions,
    keys,
    preLamports: balances.map((change) => BigInt(change.pre)),
    postLamports: balances.map((change) => BigInt(change.post)),
    fee: BigInt(meta.fee),
    tokenChanges,
  });

  return {
    signature,
    parserVersion: PARSER_VERSION,
    parserStatus: "OK",
    parsed: {
      slot: result.slot,
      blockTime: result.blockTime,
      version,
      transactionIndex: result.transactionIndex ?? null,
      recentBlockhash: message.recentBlockhash,
      signatures,
      fee: meta.fee,
      feePayer: feePayer.address,
      transactionStatus: meta.err === null ? "OK" : "ERROR",
      error: meta.err,
      computeUnitsConsumed: meta.computeUnitsConsumed ?? null,
      costUnits: meta.costUnits ?? null,
      transactionConfig: message.transactionConfig ?? null,
      accountKeys: keys,
      logs,
      logsTruncated: logs === null ? null : logs.includes(LOG_TRUNCATED),
      balanceChanges: balances,
      innerInstructionsRecorded:
        meta.innerInstructions !== null && meta.innerInstructions !== undefined,
      instructions,
      eventsAvailable: walked !== null,
      events,
      undecodedLogData,
      decodedError:
        meta.err === null ? null : decodeFailure(meta.err, walked?.failure ?? null, idls, instructions),
      idls: idlRefs,
      tokenBalanceChanges: tokenChanges,
      nativeTransfers: transfers.nativeTransfers,
      tokenTransfers: transfers.tokenTransfers,
      transfersCommitted: transfers.committed,
      transfersComplete: transfers.complete,
      unexplainedLamportChanges: transfers.unexplained,
    },
  };
}
