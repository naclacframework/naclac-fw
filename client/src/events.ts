import type { NaclacIdl } from "./idl";
import { resolveDiscriminator } from "./utils/hash";
import { decodeAccountData } from "./coder/instruction";
import type { StructLayout } from "./coder/types";
import { decodeBase64 } from "./utils/common";
import { getBase58Codec } from "@solana/codecs";

/**
 * Decodes a single "Program data: ..." log line as `eventName`, returning
 * `null` if the line isn't a program-data log or its discriminator doesn't
 * match. Shared by the Legacy and Kit clients' live subscription paths
 * (`Program.addEventListener`) and the race-free `parseEventsFromLogs` path
 * below, so there's exactly one place that knows how a Naclac event is
 * actually encoded in a log line.
 */
export function tryDecodeEventLog(
  eventDef: { fields: unknown },
  discriminator: Uint8Array,
  log: string,
  definedTypes?: readonly any[],
  isZeroCopy: StructLayout = false,
): any | null {
  if (!log.startsWith("Program data: ")) return null;

  const base64Data = log.replace("Program data: ", "").trim();
  const parts = base64Data.split(" ");
  const chunks = parts.map((p: string) => decodeBase64(p));
  const totalLength = chunks.reduce(
    (acc: number, curr: Uint8Array) => acc + curr.length,
    0,
  );
  const rawBytes = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    rawBytes.set(chunk, offset);
    offset += chunk.length;
  }

  const match =
    rawBytes.length >= discriminator.length &&
    discriminator.every((b, i) => rawBytes[i] === b);
  if (!match) return null;

  return decodeAccountData(
    eventDef.fields as any,
    new Uint8Array(rawBytes),
    definedTypes,
    isZeroCopy,
    discriminator.length,
  );
}

/**
 * Decodes every occurrence of `eventName` found in an already-fetched list of
 * transaction log lines.
 *
 * Unlike `Program.addEventListener`/`waitForEvent` (a live websocket
 * subscription — the right tool for reacting to events across many *future*
 * transactions you don't know the signature of yet, e.g. an indexer or bot),
 * this reads events out of a transaction that has already confirmed, with no
 * timing window to race: the logs already exist and aren't going anywhere.
 * This is what `.rpc()`'s returned `parseEvents()` uses to read events from
 * the transaction you just sent — mirroring the Rust SDK's
 * `tx_meta.parse_events_zero_copy()`/`parse_events_borsh()`.
 */
export function parseEventsFromLogs<T = any>(
  idl: NaclacIdl,
  eventName: string,
  logs: readonly string[],
  isZeroCopy: StructLayout = false,
): T[] {
  const eventDef = idl.events?.find((e) => e.name === eventName);
  if (!eventDef) {
    throw new Error(`[Naclac] Event "${eventName}" not found in IDL.`);
  }

  const discriminator = resolveDiscriminator(eventDef.discriminator, "event", eventName);
  const results: T[] = [];

  for (const log of logs) {
    const decoded = tryDecodeEventLog(eventDef as any, discriminator, log, idl.definedTypes, isZeroCopy);
    if (decoded !== null) results.push(decoded as T);
  }

  return results;
}

/**
 * The wire layout `eventName`'s fields are decoded with. In a zero-copy
 * program a plain `#[event]` is a fixed `repr(C)` struct (`true`), but an
 * `#[event(alloc)]` one (`allocEvents`, baked into the generated client) is
 * written field by field with `Pod` values inside (`"alloc"`). A Borsh
 * program's events are always Borsh (`false`).
 */
export function eventFieldLayout(
  isZeroCopy: boolean,
  allocEvents: readonly string[],
  eventName: string,
): StructLayout {
  if (!isZeroCopy) return false;
  return allocEvents.includes(eventName) ? "alloc" : true;
}

/**
 * A CPI-invoked instruction from a transaction's inner-instruction trace,
 * with its account keys already resolved to base58 addresses.
 */
export interface NaclacInnerInstruction {
  /** Index of the top-level instruction this CPI was made under. */
  outerIndex: number;
  /** 2 for a CPI made by a top-level instruction; 0 if the node didn't report it. */
  stackHeight: number;
  programId: string;
  accounts: readonly string[];
  data: Uint8Array;
}

/** Compiled inner instructions exactly as `getTransaction` reports them. */
export interface RpcInnerInstructions {
  index: number;
  instructions: ReadonlyArray<{
    programIdIndex: number;
    accounts: readonly number[];
    /** base58 */
    data: string;
    stackHeight?: number | null;
  }>;
}

/**
 * Resolves RPC-reported compiled inner instructions against the
 * transaction's full account-key list (static keys, then address-lookup-table
 * writable keys, then readonly keys). `undefined` means "unavailable" — the
 * node omitted them or a key couldn't be resolved — which is different from
 * `[]` (the transaction made no CPIs).
 */
export function resolveRpcInnerInstructions(
  accountKeys: readonly string[],
  list: readonly RpcInnerInstructions[] | null | undefined,
): NaclacInnerInstruction[] | undefined {
  if (!list) return undefined;
  const base58 = getBase58Codec();
  const resolved: NaclacInnerInstruction[] = [];
  for (const set of list) {
    for (const ix of set.instructions) {
      const programId = accountKeys[ix.programIdIndex];
      const accounts = ix.accounts.map((index) => accountKeys[index]);
      if (programId === undefined || accounts.some((key) => key === undefined)) {
        return undefined;
      }
      resolved.push({
        outerIndex: set.index,
        stackHeight: ix.stackHeight ?? 0,
        programId,
        accounts,
        data: Uint8Array.from(base58.encode(ix.data)),
      });
    }
  }
  return resolved;
}

/**
 * Decodes every occurrence of `eventName` that the program emitted as a
 * self-CPI (`emit_cpi!`). A call counts only if it invokes this program, its
 * first account is the program's own `event_authority` PDA (`eventAuthority`,
 * base58 — the caller derives it from seeds `["__event_authority"]`), and its
 * data starts with the IDL's `eventCpiTag`; the program's receive arm rejects
 * the call unless that PDA actually signed. Returns `[]` for a program whose
 * IDL has no `eventCpiTag`. Only pass inner instructions of a *successful*
 * transaction — a failed one's calls were never committed.
 */
export function parseEventsFromInnerInstructions<T = any>(
  idl: NaclacIdl,
  eventName: string,
  innerInstructions: readonly NaclacInnerInstruction[],
  eventAuthority: string,
  isZeroCopy: StructLayout = false,
): T[] {
  const eventDef = idl.events?.find((e) => e.name === eventName);
  if (!eventDef) {
    throw new Error(`[Naclac] Event "${eventName}" not found in IDL.`);
  }
  const tag = idl.eventCpiTag;
  if (!tag || tag.length === 0) return [];

  const discriminator = resolveDiscriminator(eventDef.discriminator, "event", eventName);
  const results: T[] = [];

  for (const call of innerInstructions) {
    if (call.programId !== idl.address || call.accounts[0] !== eventAuthority) continue;
    const data = call.data;
    if (data.length < tag.length + discriminator.length) continue;
    if (!tag.every((b, i) => data[i] === b)) continue;
    const body = data.subarray(tag.length);
    if (!discriminator.every((b, i) => body[i] === b)) continue;

    results.push(
      decodeAccountData(
        eventDef.fields as any,
        new Uint8Array(body),
        idl.definedTypes,
        isZeroCopy,
        discriminator.length,
      ) as T,
    );
  }

  return results;
}

/**
 * Decodes `eventName` from everything a confirmed transaction recorded: its
 * log lines (`emit()`), then — when `innerInstructions` and `eventAuthority`
 * are supplied — its self-CPI events (`emit_cpi!`), in that order.
 */
export function parseEventsFromTransaction<T = any>(
  idl: NaclacIdl,
  eventName: string,
  tx: {
    logs: readonly string[];
    innerInstructions?: readonly NaclacInnerInstruction[];
    eventAuthority?: string;
  },
  isZeroCopy: StructLayout = false,
): T[] {
  const fromLogs = parseEventsFromLogs<T>(idl, eventName, tx.logs, isZeroCopy);
  if (!tx.innerInstructions || tx.eventAuthority === undefined) return fromLogs;
  return [
    ...fromLogs,
    ...parseEventsFromInnerInstructions<T>(
      idl,
      eventName,
      tx.innerInstructions,
      tx.eventAuthority,
      isZeroCopy,
    ),
  ];
}

/**
 * The result of `.rpc()`: the confirmed transaction's signature and logs,
 * plus a convenience method to decode any events it emitted. Reading events
 * this way (rather than via `Program.addEventListener`/`waitForEvent`, a live
 * websocket subscription) is race-free — these logs belong to a transaction
 * that has already confirmed, so there's no timing window where an event
 * could be missed. `parseEvents` also decodes events the program emitted as
 * self-CPIs (`emit_cpi!`), which never appear in the logs; those are visible
 * only here and in `innerInstructions`, not to the live listeners.
 */
export interface NaclacRpcResult {
  signature: string;
  logs: string[];
  /** `undefined` when the backend couldn't supply them (not the same as `[]`). */
  innerInstructions?: NaclacInnerInstruction[];
  parseEvents<T = any>(eventName: string): T[];
}
