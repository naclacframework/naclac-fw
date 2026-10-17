import {
  getU8Codec,
  getU16Codec,
  getU32Codec,
  getU64Codec,
  getU128Codec,
  getI8Codec,
  getI16Codec,
  getI32Codec,
  getI64Codec,
  getI128Codec,
  getF32Codec,
  getF64Codec,
  getBooleanCodec,
  getUtf8Codec,
  getBytesCodec,
  addCodecSizePrefix,
  getArrayCodec,
  getOptionCodec,
  getDiscriminatedUnionCodec,
  getUnitCodec,
  getStructCodec as getSolanaStructCodec,
  getTupleCodec,
} from "@solana/codecs";
// getAddressCodec lives in @solana/addresses, re-exported cleanly by @solana/kit
import { getAddressCodec } from "@solana/kit";
import { toCamelCase } from "../utils/case";

/** A minimal interface for any codec from @solana/codecs / @solana/kit. */
export interface NaclacCodec {
  encode: (value: unknown) => Uint8Array | Readonly<Uint8Array> | (Uint8Array & Readonly<Uint8Array>);
  decode: (bytes: Uint8Array | Readonly<Uint8Array> | (Uint8Array & Readonly<Uint8Array>)) => unknown;
  read: (bytes: Uint8Array | Readonly<Uint8Array> | (Uint8Array & Readonly<Uint8Array>), offset: number) => [unknown, number];
}

/**
 * Maps an IDL type string (as emitted by build.rs) to its corresponding
 * @solana/codecs codec. Throws a descriptive error for unmapped types.
 *
 * Supported primitives: u8, u16, u32, u64, u128, i8, i16, i32, i64, i128,
 *                       f32, f64, bool, string, publicKey, bytes
 */
function getPrimitiveLayout(typeStr: string): { size: number } {
  switch (typeStr) {
    case "u8":
    case "i8":
    case "bool":
      return { size: 1 };
    case "u16":
    case "i16":
      return { size: 2 };
    case "u32":
    case "i32":
    case "f32":
      return { size: 4 };
    case "u64":
    case "i64":
    case "f64":
      return { size: 8 };
    case "u128":
    case "i128":
      return { size: 16 };
    case "publicKey":
      return { size: 32 };
    case "string":
    case "String":
      throw new Error("Dynamic types like string are not supported in instruction_args / Pod structs.");
    case "bytes":
      throw new Error("Dynamic bytes are not supported in instruction_args / Pod structs.");
    default:
      throw new Error(`Unknown primitive type for struct layout: ${typeStr}`);
  }
}

/**
 * The stored size of a zero-copy type. Every zero-copy layout is packed:
 * fields follow one another with no padding, so a size is the sum of its
 * parts. Never valid for a Borsh-mode type — see `getIdlCodec`'s `isZeroCopy`
 * branch.
 */
function getTypeLayout(type: any, definedTypes?: readonly any[]): { size: number } {
  if (typeof type === "string") {
    return getPrimitiveLayout(type);
  }
  if (typeof type === "object" && type !== null) {
    if ("array" in type) {
      const [innerType, count] = type.array;
      return { size: getTypeLayout(innerType, definedTypes).size * count };
    }
    if ("fixedVec" in type) {
      const [innerType, capacity] = type.fixedVec;
      return { size: FIXED_LEN_WIDTH + getTypeLayout(innerType, definedTypes).size * capacity };
    }
    if ("fixedString" in type) {
      return { size: FIXED_LEN_WIDTH + type.fixedString };
    }
    if ("defined" in type) {
      const typeName = type.defined;
      const def = definedTypes?.find((d) => d.name === typeName);
      if (def) {
        if (def.type.kind === "struct") {
          return computeStructLayout(def.type.fields, definedTypes);
        } else if (def.type.kind === "enum") {
          return { size: computeEnumLayout(def.type, definedTypes).totalSize };
        }
      }
      throw new Error(`[Naclac] The IDL references type "${typeName}" but does not define it.`);
    }
  }
  throw new Error(`Unsupported type for struct layout: ${JSON.stringify(type)}`);
}

function computeStructLayout(
  fields: any[],
  definedTypes?: readonly any[]
): { size: number; fieldsWithLayout: any[] } {
  let currentOffset = 0;
  const fieldsWithLayout: any[] = [];

  for (const field of fields) {
    const layout = getTypeLayout(field.type, definedTypes);
    fieldsWithLayout.push({
      name: field.name,
      type: field.type,
      offset: currentOffset,
      size: layout.size,
    });
    currentOffset += layout.size;
  }

  return { size: currentOffset, fieldsWithLayout };
}

/** Zero-copy-mode struct codec — fields packed in declaration order. */
function getStructCodec(fields: any[], definedTypes?: readonly any[]): NaclacCodec {
  const { size: totalSize, fieldsWithLayout } = computeStructLayout(fields, definedTypes);

  return {
    encode(value: any): Uint8Array {
      const buffer = new Uint8Array(totalSize);
      const valObj = (value ?? {}) as Record<string, any>;

      for (const field of fieldsWithLayout) {
        const codec = getIdlCodec(field.type, definedTypes, true);
        const encodedField = codec.encode(valObj[toCamelCase(field.name)]);
        buffer.set(encodedField.slice(0, field.size), field.offset);
      }

      return buffer;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const result: Record<string, any> = {};
      for (const field of fieldsWithLayout) {
        const codec = getIdlCodec(field.type, definedTypes, true);
        const [val] = codec.read(bytes, offset + field.offset);
        result[toCamelCase(field.name)] = val;
      }
      return [result, offset + totalSize];
    }
  };
}

/**
 * Borsh-mode struct codec — tightly packed, sequential, no alignment
 * padding ever (real Borsh has no C-layout concept at all), and able to
 * hold a variable-length field (`String`/`Vec<T>`/`Option<T>`) that the
 * zero-copy `getStructCodec`/`computeStructLayout` above cannot represent
 * (their fixed pre-computed offsets require every field's size to be known
 * up front). Field order is declaration order, matching real
 * `#[derive(BorshSerialize, BorshDeserialize)]` output exactly.
 */
function getBorshStructCodec(fields: any[], definedTypes?: readonly any[]): NaclacCodec {
  return {
    encode(value: any): Uint8Array {
      const valObj = (value ?? {}) as Record<string, any>;
      const parts = fields.map((f) => getIdlCodec(f.type, definedTypes, false).encode(valObj[toCamelCase(f.name)]));
      const totalLength = parts.reduce((sum, p) => sum + p.length, 0);
      const buffer = new Uint8Array(totalLength);
      let off = 0;
      for (const part of parts) {
        buffer.set(part, off);
        off += part.length;
      }
      return buffer;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const result: Record<string, any> = {};
      let off = offset;
      for (const f of fields) {
        const codec = getIdlCodec(f.type, definedTypes, false);
        const [val, newOff] = codec.read(bytes, off);
        result[toCamelCase(f.name)] = val;
        off = newOff;
      }
      return [result, off];
    },
  };
}

/**
 * Wire layout of a top-level field list: `true` = zero-copy C layout, `false`
 * = Borsh, `"alloc"` = an `#[event(alloc)]` event (see `getAllocStructCodec`).
 */
export type StructLayout = boolean | "alloc";

/**
 * Codec for an `#[event(alloc)]` event's fields: the fields themselves are
 * written back to back with no alignment gaps (`String`/`Vec` u32
 * length-prefixed, `Option` u8-tagged), but every value inside them —
 * a plain field, a `Vec` element, an `Option` payload — is a packed `Pod`
 * value in its zero-copy layout.
 */
function getAllocStructCodec(fields: any[], definedTypes?: readonly any[]): NaclacCodec {
  const fieldCodec = (type: any): NaclacCodec => {
    if (typeof type === "object" && type !== null) {
      if ("vec" in type) {
        return getArrayCodec(getIdlCodec(type.vec, definedTypes, true) as any, {
          size: getU32Codec(),
        }) as unknown as NaclacCodec;
      }
      if ("option" in type) {
        return getOptionCodec(getIdlCodec(type.option, definedTypes, true) as any, {
          prefix: getU8Codec(),
        }) as unknown as NaclacCodec;
      }
    }
    return getIdlCodec(type, definedTypes, true);
  };
  return {
    encode(value: any): Uint8Array {
      const valObj = (value ?? {}) as Record<string, any>;
      const parts = fields.map((f) => fieldCodec(f.type).encode(valObj[toCamelCase(f.name)]));
      const buffer = new Uint8Array(parts.reduce((sum, p) => sum + p.length, 0));
      let off = 0;
      for (const part of parts) {
        buffer.set(part, off);
        off += part.length;
      }
      return buffer;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const result: Record<string, any> = {};
      let off = offset;
      for (const f of fields) {
        const [val, newOff] = fieldCodec(f.type).read(bytes, off);
        result[toCamelCase(f.name)] = val;
        off = newOff;
      }
      return [result, off];
    },
  };
}

/**
 * A named field list's own codec — for a *top-level* struct-shaped field
 * list (an account's own fields, an event's own fields), not a nested
 * `{"defined": X}` reference. Exported so callers like `decodeAccountData`
 * (`coder/instruction.ts`) use the same layout logic as `getIdlCodec`'s
 * `"defined"` struct branch.
 */
export function getStructFieldsCodec(
  fields: any[],
  definedTypes: readonly any[] | undefined,
  layout: StructLayout
): NaclacCodec {
  if (layout === "alloc") return getAllocStructCodec(fields, definedTypes);
  return layout ? getStructCodec(fields, definedTypes) : getBorshStructCodec(fields, definedTypes);
}

/**
 * True when a data-carrying enum variant's `fields` array is the "named"
 * shape (`IdlField[]`, each entry `{name, type, ...}`) rather than the
 * "tuple" shape (a bare array of raw type descriptors — strings like
 * `"u64"` or defined-type objects like `{"defined": "X"}`, none of which
 * carry a top-level `name` key). Matches `naclac-idl`'s untagged
 * `IdlEnumFields::{Named, Tuple}` exactly; an empty array is treated as
 * "tuple-like" (there's nothing to distinguish either way, and both
 * branches produce the same empty result for it).
 */
function isNamedEnumFields(fields: readonly any[]): boolean {
  return fields.length > 0 && typeof fields[0] === "object" && fields[0] !== null && "name" in fields[0];
}

/**
 * Borsh/Wincode-mode enum codec — a single `u8` tag equal to the variant's
 * real Rust discriminant (explicit `= N` respected, else previous + 1),
 * matching the on-chain `#[borsh(use_discriminant = true)]` /
 * `#[wincode(tag = N)]` that `#[defined_type]` applies to every enum. The
 * IDL guarantees each discriminant fits in 0..=255 for these backends.
 *
 * `getDiscriminatedUnionCodec` itself writes the variant's array position,
 * so it is only used for the payload layout: the leading position byte is
 * swapped for the discriminant on encode and mapped back on decode. Variant
 * array order must therefore match IDL declaration order, which it does
 * (`enumDef.variants` is never reordered).
 */
function getBorshEnumCodec(enumDef: { variants: any[] }, definedTypes?: readonly any[]): NaclacCodec {
  const variantEntries = enumDef.variants.map((v: any) => {
    let codec: any;
    if (!v.fields) {
      codec = getUnitCodec();
    } else if (isNamedEnumFields(v.fields)) {
      const fieldPairs = (v.fields as any[]).map((f) => [toCamelCase(f.name), getIdlCodec(f.type, definedTypes, false) as any] as const);
      codec = getSolanaStructCodec(fieldPairs);
    } else {
      const tupleCodec = getTupleCodec((v.fields as any[]).map((ty) => getIdlCodec(ty, definedTypes, false) as any));
      codec = getSolanaStructCodec([["fields", tupleCodec] as const]);
    }
    return [v.name, codec] as const;
  });
  const discCodec = getDiscriminatedUnionCodec(variantEntries, { discriminator: "kind" }) as any;
  const discriminantByIndex: number[] = enumDef.variants.map((v: any) => Number(v.discriminant));
  const indexByDiscriminant = new Map<number, number>(discriminantByIndex.map((d, i) => [d, i]));

  const read = (bytes: Uint8Array | Readonly<Uint8Array>, offset: number): [unknown, number] => {
    const index = indexByDiscriminant.get(bytes[offset]);
    if (index === undefined) {
      throw new Error(
        `[Naclac] Invalid enum discriminant ${bytes[offset]}. Expected one of: [${discriminantByIndex.join(", ")}].`
      );
    }
    const positional = Uint8Array.from(bytes);
    positional[offset] = index;
    return discCodec.read(positional, offset) as [unknown, number];
  };

  return {
    encode(value: unknown): Uint8Array {
      const bytes = Uint8Array.from(discCodec.encode(value) as Uint8Array);
      bytes[0] = discriminantByIndex[bytes[0]];
      return bytes;
    },
    decode(bytes: Uint8Array | Readonly<Uint8Array>): unknown {
      return read(bytes, 0)[0];
    },
    read,
  };
}

/**
 * Parses an IDL enum variant's `discriminant` (a decimal string, possibly
 * negative — see `naclac_syn::types::NaclacEnumVariant::discriminant`'s
 * doc comment) into the numeric type a given tag width's codec expects:
 * `bigint` for 64-/128-bit reprs (matching `getU64Codec`/`getU128Codec`
 * etc.'s own input type), `number` otherwise.
 */
function parseDiscriminant(discriminant: string, tagType: string): number | bigint {
  return tagType === "u64" || tagType === "i64" || tagType === "u128" || tagType === "i128"
    ? BigInt(discriminant)
    : Number(discriminant);
}

interface EnumVariantCLayout {
  isUnit: boolean;
  isTuple: boolean;
  size: number;
  fieldsWithLayout: any[];
}

/**
 * A zero-copy enum's stored layout (naclac-pod's `ZcCodec`): the tag (sized
 * per `repr`, `u8` by default), then the active variant's fields one after
 * another with no padding, then zeros up to the size of the largest variant.
 * The total size is the tag plus the largest variant's fields.
 */
function computeEnumLayout(
  enumDef: { variants: any[]; repr?: string },
  definedTypes?: readonly any[]
): { tagType: string; tagSize: number; totalSize: number; variantLayouts: Map<string, EnumVariantCLayout> } {
  const tagType = enumDef.repr ?? "u8";
  const tagLayout = getPrimitiveLayout(tagType);

  let unionSize = tagLayout.size;
  const variantLayouts = new Map<string, EnumVariantCLayout>();

  for (const v of enumDef.variants) {
    if (!v.fields) {
      variantLayouts.set(v.name, { isUnit: true, isTuple: false, size: tagLayout.size, fieldsWithLayout: [] });
      continue;
    }
    const isTuple = !isNamedEnumFields(v.fields);
    const syntheticFields = isTuple
      ? [{ name: "__tag", type: tagType }, ...(v.fields as any[]).map((ty, i) => ({ name: String(i), type: ty }))]
      : [{ name: "__tag", type: tagType }, ...(v.fields as any[]).map((f) => ({ name: f.name, type: f.type }))];
    const layout = computeStructLayout(syntheticFields, definedTypes);
    variantLayouts.set(v.name, {
      isUnit: false,
      isTuple,
      size: layout.size,
      // Drop the synthetic leading `__tag` entry — callers only need the
      // real fields, at the offsets `computeStructLayout` already
      // resolved relative to the start of the variant's payload (which is
      // the same as the start of the whole encoded value, offset 0).
      fieldsWithLayout: layout.fieldsWithLayout.slice(1),
    });
    if (layout.size > unionSize) unionSize = layout.size;
  }

  return { tagType, tagSize: tagLayout.size, totalSize: unionSize, variantLayouts };
}

/**
 * Fixed-length array of elements whose codec is one of this file's
 * layout codecs (zero-copy struct/enum), which expose `encode`/`read` but not
 * the `write`/size protocol `getArrayCodec` requires. Elements sit back to
 * back; each element's encoded length is its own `size_of`.
 */
function getFixedArrayOfLayoutCodec(inner: NaclacCodec, count: number): NaclacCodec {
  return {
    encode(value: any): Uint8Array {
      const items = value as unknown[];
      if (!Array.isArray(items) || items.length !== count) {
        throw new Error(`[Naclac] Expected an array of exactly ${count} elements.`);
      }
      const parts = items.map((item) => inner.encode(item) as Uint8Array);
      const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
      let at = 0;
      for (const part of parts) {
        out.set(part, at);
        at += part.length;
      }
      return out;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const items: unknown[] = [];
      let at = offset;
      for (let i = 0; i < count; i++) {
        const [item, next] = inner.read(bytes, at);
        items.push(item);
        at = next;
      }
      return [items, at];
    },
  };
}

/** Width in bytes of the little-endian length field of a `FixedVec` or `FixedString`. */
const FIXED_LEN_WIDTH = 2;

function readFixedLen(bytes: Uint8Array, offset: number, width: number): number {
  let len = 0n;
  for (let i = width - 1; i >= 0; i--) {
    len = (len << 8n) | BigInt(bytes[offset + i]);
  }
  return len > BigInt(Number.MAX_SAFE_INTEGER) ? Number.MAX_SAFE_INTEGER : Number(len);
}

function writeFixedLen(out: Uint8Array, width: number, len: number): void {
  let rest = BigInt(len);
  for (let i = 0; i < width; i++) {
    out[i] = Number(rest & 0xffn);
    rest >>= 8n;
  }
}

/**
 * Codec for the account-storage `FixedVec<T, N>`: a little-endian length
 * followed by all `N` element slots. Decoding yields the first `len` items,
 * with a stored length above `N` read as `N`, as the program does.
 */
function getFixedVecCodec(inner: NaclacCodec, innerLayout: { size: number }, capacity: number): NaclacCodec {
  const lenWidth = FIXED_LEN_WIDTH;
  const totalSize = lenWidth + capacity * innerLayout.size;
  return {
    encode(value: any): Uint8Array {
      const items = value as unknown[];
      if (!Array.isArray(items) || items.length > capacity) {
        throw new Error(`[Naclac] Expected an array of at most ${capacity} elements.`);
      }
      const out = new Uint8Array(totalSize);
      writeFixedLen(out, lenWidth, items.length);
      items.forEach((item, i) => {
        const part = inner.encode(item) as Uint8Array;
        out.set(part.slice(0, innerLayout.size), lenWidth + i * innerLayout.size);
      });
      return out;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const len = Math.min(readFixedLen(bytes, offset, lenWidth), capacity);
      const items: unknown[] = [];
      for (let i = 0; i < len; i++) {
        const [item] = inner.read(bytes, offset + lenWidth + i * innerLayout.size);
        items.push(item);
      }
      return [items, offset + totalSize];
    },
  };
}

/** Codec for the account-storage `FixedString<N>`: a two-byte length followed by `N` UTF-8 byte slots. */
function getFixedStringCodec(capacity: number): NaclacCodec {
  const totalSize = FIXED_LEN_WIDTH + capacity;
  return {
    encode(value: any): Uint8Array {
      if (typeof value !== "string") {
        throw new Error("[Naclac] Expected a string.");
      }
      const bytes = new TextEncoder().encode(value);
      if (bytes.length > capacity) {
        throw new Error(`[Naclac] String is ${bytes.length} bytes; the field holds at most ${capacity}.`);
      }
      const out = new Uint8Array(totalSize);
      writeFixedLen(out, 2, bytes.length);
      out.set(bytes, 2);
      return out;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const len = Math.min(readFixedLen(bytes, offset, 2), capacity);
      const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes.slice(offset + 2, offset + 2 + len));
      return [text, offset + totalSize];
    },
  };
}

function concatBytes(parts: readonly Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

/** `Bool` in a zero-copy layout: written as 0 or 1, read as true for any non-zero byte. */
const zeroCopyBoolCodec: NaclacCodec = {
  encode: (value: unknown) => Uint8Array.of(value ? 1 : 0),
  decode: (bytes) => bytes[0] !== 0,
  read: (bytes, offset) => [bytes[offset] !== 0, offset + 1],
};

/** True for an IDL type the zero-copy program reads with a length prefix or a tag instead of a fixed layout. */
function isVariableArgType(type: any): boolean {
  return type === "string" || type === "bytes" || (typeof type === "object" && type !== null && ("vec" in type || "option" in type));
}

/**
 * Wire format of one instruction argument in a zero-copy program, as the
 * program reads it (`NaclacArgs` / `NaclacPod`): arguments and struct fields
 * are packed back to back with no alignment padding between them.
 *
 * - `Vec<T>` / `ZcVec<T>`: a u32 LE *byte* length, then each element in its
 *   zero-copy layout (not an element count, unlike an `#[event(alloc)]`).
 * - `String` / `ZcString`: a u32 LE byte length, then UTF-8.
 * - `Option<T>`: a one-byte tag, then `T`'s bytes — zeroes of `T`'s size when
 *   `None`, so the argument always occupies `1 + size_of::<T>()` bytes.
 * - A `#[instruction_args]` struct holding any of the above is read field by
 *   field in declaration order; a struct of fixed fields only is one packed
 *   block, which `getIdlCodec` already writes.
 */
export function getZeroCopyArgCodec(type: any, definedTypes: readonly any[] | undefined): NaclacCodec {
  if (typeof type === "object" && type !== null) {
    if ("vec" in type) {
      return getZeroCopyArgVecCodec(type.vec, definedTypes);
    }
    if ("option" in type) {
      return getZeroCopyArgOptionCodec(type.option, definedTypes);
    }
    if ("defined" in type) {
      const def = definedTypes?.find((d) => d.name === type.defined);
      if (def?.type.kind === "struct" && def.type.fields.some((f: any) => isVariableArgType(f.type))) {
        return getZeroCopyArgStructCodec(def.type.fields, definedTypes);
      }
    }
  }
  return getIdlCodec(type, definedTypes, true);
}

function getZeroCopyArgVecCodec(elementType: any, definedTypes: readonly any[] | undefined): NaclacCodec {
  const element = getIdlCodec(elementType, definedTypes, true);

  // `Vec<String>` / `Vec<Vec<u8>>`: items have no fixed size, so the prefix is an element
  // count and each item carries its own u32 byte length (what the program's reader expects).
  const hasVariableItems =
    elementType === "string" ||
    (typeof elementType === "object" && elementType !== null && elementType.vec === "u8");
  if (hasVariableItems) {
    return {
      encode(value: unknown): Uint8Array {
        const items = Array.from(value as ArrayLike<unknown>);
        const prefix = new Uint8Array(4);
        new DataView(prefix.buffer).setUint32(0, items.length, true);
        return concatBytes([prefix, ...items.map((item) => element.encode(item) as Uint8Array)]);
      },
      decode(bytes: Uint8Array): any {
        return this.read(bytes, 0)[0];
      },
      read(bytes: Uint8Array, offset: number): [any, number] {
        const count = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, true);
        const items: unknown[] = [];
        let at = offset + 4;
        for (let i = 0; i < count; i++) {
          const [item, next] = element.read(bytes, at);
          items.push(item);
          at = next;
        }
        return [items, at];
      },
    };
  }

  return {
    encode(value: unknown): Uint8Array {
      const items = Array.from(value as ArrayLike<unknown>);
      const body = concatBytes(items.map((item) => element.encode(item) as Uint8Array));
      const prefix = new Uint8Array(4);
      new DataView(prefix.buffer).setUint32(0, body.length, true);
      return concatBytes([prefix, body]);
    },
    decode(bytes: Uint8Array): any {
      return this.read(bytes, 0)[0];
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const byteLength = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, true);
      const end = offset + 4 + byteLength;
      const items: unknown[] = [];
      let at = offset + 4;
      while (at < end) {
        const [item, next] = element.read(bytes, at);
        items.push(item);
        at = next;
      }
      return [items, end];
    },
  };
}

function getZeroCopyArgOptionCodec(innerType: any, definedTypes: readonly any[] | undefined): NaclacCodec {
  const inner = getIdlCodec(innerType, definedTypes, true);
  const innerSize = getTypeLayout(innerType, definedTypes).size;
  return {
    encode(value: any): Uint8Array {
      const isOptionObject = typeof value === "object" && value !== null && "__option" in value;
      const isNone = value === null || value === undefined || (isOptionObject && value.__option === "None");
      if (isNone) {
        return new Uint8Array(1 + innerSize);
      }
      const present = isOptionObject ? value.value : value;
      return concatBytes([Uint8Array.of(1), inner.encode(present) as Uint8Array]);
    },
    decode(bytes: Uint8Array): any {
      return this.read(bytes, 0)[0];
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const tag = bytes[offset];
      const end = offset + 1 + innerSize;
      if (tag === 0) {
        return [{ __option: "None" }, end];
      }
      if (tag !== 1) {
        throw new Error(`[Naclac] Invalid Option tag ${tag}; expected 0 or 1.`);
      }
      const [value] = inner.read(bytes, offset + 1);
      return [{ __option: "Some", value }, end];
    },
  };
}

function getZeroCopyArgStructCodec(fields: any[], definedTypes: readonly any[] | undefined): NaclacCodec {
  const members = fields.map((field) => ({
    key: toCamelCase(field.name),
    codec: getZeroCopyArgCodec(field.type, definedTypes),
  }));
  return {
    encode(value: any): Uint8Array {
      return concatBytes(members.map(({ key, codec }) => codec.encode(value?.[key]) as Uint8Array));
    },
    decode(bytes: Uint8Array): any {
      return this.read(bytes, 0)[0];
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const result: Record<string, unknown> = {};
      let at = offset;
      for (const { key, codec } of members) {
        const [value, next] = codec.read(bytes, at);
        result[key] = value;
        at = next;
      }
      return [result, at];
    },
  };
}

/**
 * Zero-copy-mode enum codec — tag plus the active variant's packed fields
 * (see `computeEnumLayout`). The tag written and matched is the variant's
 * real discriminant (`variant.discriminant`), not its positional index, the
 * opposite of `getBorshEnumCodec`.
 */
function getZeroCopyEnumCodec(
  enumDef: { variants: any[]; repr?: string },
  definedTypes?: readonly any[],
  enumName?: string
): NaclacCodec {
  const { tagType, totalSize, variantLayouts } = computeEnumLayout(enumDef, definedTypes);
  const tagCodec = getIdlCodec(tagType, definedTypes, true);

  function fieldValueFor(layout: EnumVariantCLayout, field: any, valObj: Record<string, any>): unknown {
    return layout.isTuple ? (valObj.fields ?? [])[Number(field.name)] : valObj[toCamelCase(field.name)];
  }

  return {
    encode(value: any): Uint8Array {
      const buffer = new Uint8Array(totalSize);
      const valObj = (value ?? {}) as Record<string, any>;
      const variant = enumDef.variants.find((v: any) => v.name === valObj.kind);
      if (!variant) {
        throw new Error(
          `[Naclac] Unknown enum variant "${valObj.kind}". Expected one of: [${enumDef.variants.map((v: any) => v.name).join(", ")}].`
        );
      }
      const tagBytes = tagCodec.encode(parseDiscriminant(variant.discriminant, tagType) as any) as Uint8Array;
      buffer.set(tagBytes, 0);

      const layout = variantLayouts.get(variant.name)!;
      for (const field of layout.fieldsWithLayout) {
        const codec = getIdlCodec(field.type, definedTypes, true);
        const encoded = codec.encode(fieldValueFor(layout, field, valObj)) as Uint8Array;
        buffer.set(encoded.slice(0, field.size), field.offset);
      }
      return buffer;
    },
    decode(bytes: Uint8Array): any {
      const [val] = this.read(bytes, 0);
      return val;
    },
    read(bytes: Uint8Array, offset: number): [any, number] {
      const [tagValue] = tagCodec.read(bytes, offset);
      const variant = enumDef.variants.find((v: any) => v.discriminant === String(tagValue));
      if (!variant) {
        const unsetHint =
          String(tagValue) === "0"
            ? ` A tag of 0 usually means the field was never set (a zeroed slot) in an enum with no variant 0; ` +
              `give the enum a variant 0 or set the field when the account is initialized.`
            : "";
        throw new Error(
          `[Naclac] Invalid enum discriminant ${String(tagValue)} while decoding${enumName ? ` ${enumName}` : ""} — ` +
          `no variant of this enum declares it. This indicates corrupted account/instruction data, not a client ` +
          `bug.${unsetHint}`
        );
      }
      const layout = variantLayouts.get(variant.name)!;
      if (layout.isUnit) {
        return [{ kind: variant.name }, offset + totalSize];
      }
      if (layout.isTuple) {
        const fieldsArr: unknown[] = [];
        for (const field of layout.fieldsWithLayout) {
          const codec = getIdlCodec(field.type, definedTypes, true);
          const [v] = codec.read(bytes, offset + field.offset);
          fieldsArr[Number(field.name)] = v;
        }
        return [{ kind: variant.name, fields: fieldsArr }, offset + totalSize];
      }
      const result: Record<string, unknown> = { kind: variant.name };
      for (const field of layout.fieldsWithLayout) {
        const codec = getIdlCodec(field.type, definedTypes, true);
        const [v] = codec.read(bytes, offset + field.offset);
        result[toCamelCase(field.name)] = v;
      }
      return [result, offset + totalSize];
    },
  };
}

/**
 * Maps an IDL type to its corresponding codec. `isZeroCopy` (from the
 * program's own `NaclacIdl.isZeroCopy`) selects the wire layout for any
 * `{"defined": X}` reference: the packed zero-copy layout when `true`,
 * Borsh when `false`. The two are not interchangeable: a Borsh struct with a
 * `String`/`Vec` field cannot be represented in the zero-copy layout, and a
 * zero-copy enum stores its payload padded to a fixed size. The primitive
 * cases below are the same in both modes, except `bool`, which a zero-copy
 * layout reads as true for any non-zero byte.
 *
 * Supported primitives: u8, u16, u32, u64, u128, i8, i16, i32, i64, i128,
 *                       f32, f64, bool, string, publicKey, bytes
 */
export function getIdlCodec(type: any, definedTypes: readonly any[] | undefined, isZeroCopy: boolean): NaclacCodec {
  if (typeof type === "string") {
    switch (type) {
      case "u8":
        return getU8Codec() as unknown as NaclacCodec;
      case "u16":
        return getU16Codec() as unknown as NaclacCodec;
      case "u32":
        return getU32Codec() as unknown as NaclacCodec;
      case "u64":
        return getU64Codec() as unknown as NaclacCodec;
      case "u128":
        return getU128Codec() as unknown as NaclacCodec;
      case "i8":
        return getI8Codec() as unknown as NaclacCodec;
      case "i16":
        return getI16Codec() as unknown as NaclacCodec;
      case "i32":
        return getI32Codec() as unknown as NaclacCodec;
      case "i64":
        return getI64Codec() as unknown as NaclacCodec;
      case "i128":
        return getI128Codec() as unknown as NaclacCodec;
      case "f32":
        return getF32Codec() as unknown as NaclacCodec;
      case "f64":
        return getF64Codec() as unknown as NaclacCodec;
      case "bool":
        return isZeroCopy ? zeroCopyBoolCodec : (getBooleanCodec() as unknown as NaclacCodec);
      case "string":
      case "String":
        // Borsh dynamic string: 4-byte LE length prefix + UTF-8 bytes
        return addCodecSizePrefix(getUtf8Codec(), getU32Codec()) as unknown as NaclacCodec;
      case "publicKey":
        return getAddressCodec() as unknown as NaclacCodec;
      case "bytes":
        return getBytesCodec() as unknown as NaclacCodec;
      default:
        throw new Error(
          `[Naclac] Unknown IDL type "${type}". ` +
          `Supported types: u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, ` +
          `f32, f64, bool, string, publicKey, bytes.`
        );
    }
  }

  if (typeof type === "object" && type !== null) {
    if ("array" in type) {
      const [innerType, size] = type.array;

      // Special case: [u8; N] — support plain string input with auto-padding
      if (innerType === "u8" && typeof size === "number") {
        const innerCodec = getIdlCodec(innerType, definedTypes, isZeroCopy) as any;
        const rawCodec = getArrayCodec(innerCodec, { size }) as unknown as NaclacCodec;
        return {
          encode: (value: unknown) => {
            if (typeof value === "string") {
              // Auto-pad / truncate string to fixed size, space-padded
               const buf = new Uint8Array(size);
               const encoded = new TextEncoder().encode(value);
               buf.set(encoded.slice(0, size));
               return buf;
            }
            return rawCodec.encode(value);
          },
          decode: rawCodec.decode,
          read: rawCodec.read,
        };
      }

      const innerCodec = getIdlCodec(innerType, definedTypes, isZeroCopy) as any;
      if (typeof size === "number" && typeof innerCodec.write !== "function") {
        return getFixedArrayOfLayoutCodec(innerCodec, size);
      }
      return getArrayCodec(innerCodec, { size }) as unknown as NaclacCodec;
    }
    if ("fixedVec" in type) {
      const [innerType, capacity] = type.fixedVec;
      return getFixedVecCodec(
        getIdlCodec(innerType, definedTypes, true),
        getTypeLayout(innerType, definedTypes),
        capacity
      );
    }
    if ("fixedString" in type) {
      return getFixedStringCodec(type.fixedString);
    }
    if ("vec" in type) {
      const innerType = type.vec;
      // Borsh `Vec<T>`: a u32 LE element count, then the elements. The array
      // codec already writes that count, so it must not be wrapped in a
      // second (byte-size) prefix.
      return getArrayCodec(getIdlCodec(innerType, definedTypes, isZeroCopy) as any, {
        size: getU32Codec(),
      }) as unknown as NaclacCodec;
    }
    if ("option" in type) {
      const innerType = type.option;
      return getOptionCodec(getIdlCodec(innerType, definedTypes, isZeroCopy) as any, { prefix: getU8Codec() }) as unknown as NaclacCodec;
    }
    if ("defined" in type) {
      const typeName = type.defined;
      const def = definedTypes?.find((d) => d.name === typeName);
      if (def) {
        if (def.type.kind === "struct") {
          return isZeroCopy
            ? getStructCodec(def.type.fields, definedTypes)
            : getBorshStructCodec(def.type.fields, definedTypes);
        } else if (def.type.kind === "enum") {
          return isZeroCopy
            ? getZeroCopyEnumCodec(def.type, definedTypes, typeName)
            : getBorshEnumCodec(def.type, definedTypes);
        }
      }
      throw new Error(`[Naclac] The IDL references type "${typeName}" but does not define it.`);
    }
  }

  throw new Error(`[Naclac] Unknown IDL type "${JSON.stringify(type)}". Supported types include primitives, array, vec, option.`);
}
