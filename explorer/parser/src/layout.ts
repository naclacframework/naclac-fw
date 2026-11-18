/**
 * Decoding of IDL field lists from bytes. Supports the fixed-width types whose
 * layout is identical in the borsh, wincode and bytemuck serializations (every
 * zero-copy layout is packed, with no padding): integers, floats, bool and
 * publicKey. Any other type is reported as unsupported, never guessed.
 */

import { encodeBase58 } from "./base58.ts";
import type { IdlField, Serialization } from "./idl.ts";

export type DecodedValue = string | number | boolean;

export type FieldsResult =
  | { ok: true; values: Record<string, DecodedValue>; end: number }
  | { ok: false; status: "unsupported-type" | "decode-error"; reason: string };

interface FixedType {
  size: number;
  read: (view: DataView, offset: number, serialization: Serialization) => DecodedValue | null;
}

function u128(view: DataView, offset: number): bigint {
  return (view.getBigUint64(offset + 8, true) << 64n) | view.getBigUint64(offset, true);
}

function finite(value: number): DecodedValue {
  return Number.isFinite(value) ? value : String(value);
}

const FIXED = new Map<string, FixedType>([
  ["u8", { size: 1, read: (v, o) => v.getUint8(o) }],
  ["u16", { size: 2, read: (v, o) => v.getUint16(o, true) }],
  ["u32", { size: 4, read: (v, o) => v.getUint32(o, true) }],
  ["u64", { size: 8, read: (v, o) => v.getBigUint64(o, true).toString() }],
  ["u128", { size: 16, read: (v, o) => u128(v, o).toString() }],
  ["i8", { size: 1, read: (v, o) => v.getInt8(o) }],
  ["i16", { size: 2, read: (v, o) => v.getInt16(o, true) }],
  ["i32", { size: 4, read: (v, o) => v.getInt32(o, true) }],
  ["i64", { size: 8, read: (v, o) => v.getBigInt64(o, true).toString() }],
  ["i128", { size: 16, read: (v, o) => BigInt.asIntN(128, u128(v, o)).toString() }],
  ["f32", { size: 4, read: (v, o) => finite(v.getFloat32(o, true)) }],
  ["f64", { size: 8, read: (v, o) => finite(v.getFloat64(o, true)) }],
  [
    "bool",
    {
      size: 1,
      read: (v, o, serialization) => {
        const byte = v.getUint8(o);
        if (serialization === "bytemuck") return byte !== 0;
        return byte === 0 || byte === 1 ? byte === 1 : null;
      },
    },
  ],
  [
    "publicKey",
    {
      size: 32,
      read: (v, o) => encodeBase58(new Uint8Array(v.buffer.slice(v.byteOffset + o, v.byteOffset + o + 32))),
    },
  ],
]);

function shorten(value: string, limit: number): string {
  return value.length > limit ? `${value.slice(0, limit)}...` : value;
}

/**
 * Decodes `fields` in order from `bytes`, starting at byte `start`. Returns the
 * values (as a null-prototype object keyed by the IDL's field names) and the
 * offset after the last field, or the reason decoding stopped.
 */
export function decodeFields(
  fields: IdlField[],
  bytes: Uint8Array,
  start: number,
  serialization: Serialization,
): FieldsResult {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values: Record<string, DecodedValue> = Object.create(null);
  let offset = start;
  for (const field of fields) {
    const fixed = typeof field.type === "string" ? FIXED.get(field.type) : undefined;
    const name = shorten(field.name, 40);
    if (fixed === undefined) {
      return {
        ok: false,
        status: "unsupported-type",
        reason: `field "${name}" has type ${shorten(String(JSON.stringify(field.type)), 60)}, which is not decoded yet.`,
      };
    }
    if (offset + fixed.size > bytes.length) {
      return {
        ok: false,
        status: "decode-error",
        reason: `field "${name}" needs ${fixed.size} bytes at offset ${offset}, but the data has ${bytes.length}.`,
      };
    }
    const value = fixed.read(view, offset, serialization);
    if (value === null) {
      return { ok: false, status: "decode-error", reason: `field "${name}" holds an invalid bool byte.` };
    }
    values[field.name] = value;
    offset += fixed.size;
  }
  return { ok: true, values, end: offset };
}
