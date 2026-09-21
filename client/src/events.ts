import type { NaclacIdl } from "./idl";
import { getDiscriminator } from "./utils/hash";
import { decodeAccountData } from "./coder/instruction";
import { decodeBase64 } from "./utils/common";

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
  isZeroCopy = false,
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

  const match = rawBytes.slice(0, 8).every((b, i) => b === discriminator[i]);
  if (!match) return null;

  return decodeAccountData(eventDef.fields as any, new Uint8Array(rawBytes), definedTypes, isZeroCopy);
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
  isZeroCopy = false,
): T[] {
  const eventDef = idl.events?.find((e) => e.name === eventName);
  if (!eventDef) {
    throw new Error(`[Naclac] Event "${eventName}" not found in IDL.`);
  }

  const discriminator = getDiscriminator("event", eventName);
  const results: T[] = [];

  for (const log of logs) {
    const decoded = tryDecodeEventLog(eventDef as any, discriminator, log, idl.definedTypes, isZeroCopy);
    if (decoded !== null) results.push(decoded as T);
  }

  return results;
}

/**
 * The result of `.rpc()`: the confirmed transaction's signature and logs,
 * plus a convenience method to decode any events it emitted. Reading events
 * this way (rather than via `Program.addEventListener`/`waitForEvent`, a live
 * websocket subscription) is race-free — these logs belong to a transaction
 * that has already confirmed, so there's no timing window where an event
 * could be missed.
 */
export interface NaclacRpcResult {
  signature: string;
  logs: string[];
  parseEvents<T = any>(eventName: string): T[];
}
