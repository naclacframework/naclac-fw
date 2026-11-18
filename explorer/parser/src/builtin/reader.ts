/**
 * A bounds-checked reader over instruction data, and the result type every
 * built-in program decoder returns. A read past the end, or a value the
 * program's own decoder would refuse, ends decoding with a reason; nothing is
 * read out of range or guessed.
 */

import { encodeBase58 } from "../base58.ts";
import type { ArgValue } from "../types.ts";

export type BuiltinDecode =
  | {
      ok: true;
      name: string;
      args: Record<string, ArgValue>;
      accounts: string[];
      trailingBytes: number;
    }
  | {
      ok: false;
      status: "unknown-instruction" | "decode-error" | "unsupported-type";
      name: string | null;
      reason: string;
    };

/** A built-in decoder: the instruction data and how many accounts the instruction has. */
export type BuiltinDecoder = (data: Uint8Array, accountCount: number) => BuiltinDecode;

export class DecodeFailure extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DecodeFailure";
  }
}

export class Reader {
  readonly bytes: Uint8Array;
  private readonly view: DataView;
  offset: number;

  constructor(bytes: Uint8Array, offset: number) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    this.offset = offset;
  }

  get remaining(): number {
    return this.bytes.length - this.offset;
  }

  private need(count: number, what: string): void {
    if (this.remaining < count) {
      throw new DecodeFailure(
        `${what} needs ${count} bytes at offset ${this.offset}, but ${this.remaining} remain.`,
      );
    }
  }

  u8(what: string): number {
    this.need(1, what);
    return this.view.getUint8(this.offset++);
  }

  u16(what: string): number {
    this.need(2, what);
    const value = this.view.getUint16(this.offset, true);
    this.offset += 2;
    return value;
  }

  u32(what: string): number {
    this.need(4, what);
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  u64(what: string): string {
    this.need(8, what);
    const value = this.view.getBigUint64(this.offset, true);
    this.offset += 8;
    return value.toString();
  }

  pubkey(what: string): string {
    this.need(32, what);
    const key = encodeBase58(this.bytes.slice(this.offset, this.offset + 32));
    this.offset += 32;
    return key;
  }

  /** An option stored as a 1-byte flag (0 or 1) followed by the value when the flag is 1. */
  optionPubkey(what: string): string | null {
    const flag = this.u8(`${what} flag`);
    if (flag === 0) return null;
    if (flag !== 1) throw new DecodeFailure(`${what} flag is ${flag}, not 0 or 1.`);
    return this.pubkey(what);
  }

  optionU64(what: string): string | null {
    const flag = this.u8(`${what} flag`);
    if (flag === 0) return null;
    if (flag !== 1) throw new DecodeFailure(`${what} flag is ${flag}, not 0 or 1.`);
    return this.u64(what);
  }

  /** A string stored as a u64 length followed by that many UTF-8 bytes. */
  string64(what: string): string {
    this.need(8, `${what} length`);
    const length = this.view.getBigUint64(this.offset, true);
    this.offset += 8;
    if (length > BigInt(this.remaining)) {
      throw new DecodeFailure(`${what} declares ${length} bytes, but ${this.remaining} remain.`);
    }
    return utf8(this.take(Number(length)), what);
  }

  take(count: number): Uint8Array {
    const slice = this.bytes.subarray(this.offset, this.offset + count);
    this.offset += count;
    return slice;
  }

  rest(): Uint8Array {
    return this.take(this.remaining);
  }
}

/** Decodes UTF-8, throwing a DecodeFailure that names `what` when the bytes are not valid UTF-8. */
export function utf8(bytes: Uint8Array, what: string): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new DecodeFailure(`${what} is not valid UTF-8.`);
  }
}

/**
 * Runs `body` over `data` from byte `start` and returns the instruction it
 * decoded, with the number of bytes left unread, or the reason it failed.
 */
export function decodeWith(
  name: string,
  data: Uint8Array,
  start: number,
  body: (reader: Reader) => { args: Record<string, ArgValue>; accounts: string[] },
): BuiltinDecode {
  const reader = new Reader(data, start);
  try {
    const { args, accounts } = body(reader);
    return { ok: true, name, args, accounts, trailingBytes: reader.remaining };
  } catch (error) {
    if (error instanceof DecodeFailure) {
      return { ok: false, status: "decode-error", name, reason: error.message };
    }
    throw error;
  }
}

export function unknownInstruction(reason: string): BuiltinDecode {
  return { ok: false, status: "unknown-instruction", name: null, reason };
}
