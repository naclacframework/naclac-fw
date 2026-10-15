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
function getPrimitiveLayout(typeStr: string): { size: number; alignment: number } {
  switch (typeStr) {
    case "u8":
    case "i8":
    case "bool":
      return { size: 1, alignment: 1 };
    case "u16":
    case "i16":
      return { size: 2, alignment: 2 };
    case "u32":
    case "i32":
    case "f32":
      return { size: 4, alignment: 4 };
    case "u64":
    case "i64":
    case "f64":
      return { size: 8, alignment: 8 };
    case "u128":
    case "i128":
      return { size: 16, alignment: 8 }; // On SBF/BPF target, alignment of u128 is 8 bytes
    case "publicKey":
      return { size: 32, alignment: 1 }; // Pubkey wraps [u8; 32], alignment is 1
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
 * A zero-copy type's real C-layout size/alignment — used only on the
 * zero-copy path (real `#[repr(C)]`/`#[repr(uN)]` layout with alignment
 * padding). Never valid for a Borsh-mode type, which is tightly packed with
 * no alignment concept at all — see `getIdlCodec`'s `isZeroCopy` branch.
 */
function getTypeLayout(type: any, definedTypes?: readonly any[]): { size: number; alignment: number } {
  if (typeof type === "string") {
    return getPrimitiveLayout(type);
  }
  if (typeof type === "object" && type !== null) {
    if ("array" in type) {
      const [innerType, count] = type.array;
      const innerLayout = getTypeLayout(innerType, definedTypes);
      return { size: innerLayout.size * count, alignment: innerLayout.alignment };
    }
    if ("defined" in type) {
      const typeName = type.defined;
      const def = definedTypes?.find((d) => d.name === typeName);
      if (def) {
        if (def.type.kind === "struct") {
          return computeStructLayout(def.type.fields, definedTypes);
        } else if (def.type.kind === "enum") {
          const { totalSize, unionAlignment } = computeEnumCLayout(def.type, definedTypes);
          return { size: totalSize, alignment: unionAlignment };
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
): { size: number; alignment: number; fieldsWithLayout: any[] } {
  let currentOffset = 0;
  let maxAlign = 1;
  const fieldsWithLayout: any[] = [];

  for (const field of fields) {
    const layout = getTypeLayout(field.type, definedTypes);
    const align = layout.alignment;

    // C-layout alignment: offset must be a multiple of the field's alignment
    const paddingNeeded = (align - (currentOffset % align)) % align;
    currentOffset += paddingNeeded;

    fieldsWithLayout.push({
      name: field.name,
      type: field.type,
      offset: currentOffset,
      size: layout.size,
      alignment: align,
    });

    currentOffset += layout.size;
    if (align > maxAlign) {
      maxAlign = align;
    }
  }

  // C-layout padding: struct size must be a multiple of its overall alignment
  const tailPaddingNeeded = (maxAlign - (currentOffset % maxAlign)) % maxAlign;
  const totalSize = currentOffset + tailPaddingNeeded;

  return {
    size: totalSize,
    alignment: maxAlign,
    fieldsWithLayout,
  };
}

/** Zero-copy-mode struct codec — real C-layout, alignment padding included. */
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
 * a plain field, a `Vec` element, an `Option` payload — is a raw `Pod` value
 * in its C layout, so a nested `defined` type keeps its internal padding.
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
 * (`coder/instruction.ts`) can reuse the exact same alignment-aware layout
 * logic `getIdlCodec`'s `"defined"` struct branch already uses for a
 * *nested* struct, instead of a naive sequential walk that has no way to
 * know about a zero-copy account's own internal alignment gaps (real,
 * `#[repr(C)]`-driven gaps between the account's own top-level fields —
 * `naclac-syn`'s IDL never lists them by name, since `pod_struct_checks`'s
 * auto-inserted padding only exists in the macro-expanded on-chain code,
 * never in the source text `naclac-syn` parses; see
 * `docs/plan/anchor-idl-conversion-and-enum-pod-audit.md`'s Gap #2 final
 * status for the full reasoning). Borsh mode has no such concern (no
 * alignment gaps ever), so it stays the same tightly-sequential codec
 * either way.
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
 * Computes a zero-copy enum's real on-chain layout: tag first (sized per
 * `repr`, defaulting to `u8` exactly like `#[defined_type]`'s own default —
 * `macros/src/lib.rs`), then each variant's own fields laid out
 * exactly like a struct whose first field is the tag (the same trick
 * `macros/src/checked_enum.rs`'s payload structs use, and the same
 * reason it's sound — see that file's own comment citing the Rust
 * reference on primitive representation of enums with fields), and an
 * overall union size/alignment = the max over every variant's own
 * (already self-padded) size/alignment, itself rounded up to that overall
 * alignment (standard `#[repr(C)]` union sizing). This mirrors
 * `naclac-pod-codegen`'s — the Rust client's copy of `macros/src/
 * checked_enum.rs` — own algorithm field-for-field; both must agree, since
 * both are independent implementations of what the real Rust compiler
 * computes for the on-chain enum, not derived from each other.
 */
function computeEnumCLayout(
  enumDef: { variants: any[]; repr?: string },
  definedTypes?: readonly any[]
): { tagType: string; tagSize: number; totalSize: number; unionAlignment: number; variantLayouts: Map<string, EnumVariantCLayout> } {
  const tagType = enumDef.repr ?? "u8";
  const tagLayout = getPrimitiveLayout(tagType);

  let unionSize = tagLayout.size;
  let unionAlignment = tagLayout.alignment;
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
    if (layout.alignment > unionAlignment) unionAlignment = layout.alignment;
  }

  const rem = unionSize % unionAlignment;
  const totalSize = rem === 0 ? unionSize : unionSize + (unionAlignment - rem);

  return { tagType, tagSize: tagLayout.size, totalSize, unionAlignment, variantLayouts };
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

function concatBytes(parts: readonly Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

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
 *   field in declaration order; a struct of fixed fields only is one
 *   `repr(C)` block with alignment padding, which `getIdlCodec` already writes.
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
 * Zero-copy-mode enum codec — real C-layout (tag + per-variant fields with
 * alignment padding, see `computeEnumCLayout`), and the tag byte(s) written/
 * matched are the variant's *real* discriminant (`variant.discriminant`),
 * not its positional index — the opposite convention from
 * `getBorshEnumCodec`, and correct for zero-copy mode specifically because
 * `macros/src/checked_enum.rs`'s `variant_discriminants` mirrors
 * Rust's own real compiler discriminant rule (explicit `= N` respected),
 * unlike Borsh's derive default.
 */
function getZeroCopyEnumCodec(
  enumDef: { variants: any[]; repr?: string },
  definedTypes?: readonly any[],
  enumName?: string
): NaclacCodec {
  const { tagType, totalSize, variantLayouts } = computeEnumCLayout(enumDef, definedTypes);
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
 * `{"defined": X}` reference: real C-layout with alignment padding when
 * `true`, tightly-packed Borsh when `false` — the two are not
 * interchangeable (a zero-copy struct/enum with any internal padding gap
 * would encode/decode wrong bytes under the Borsh path, and vice versa for
 * a Borsh struct with a `String`/`Vec` field, which the zero-copy path
 * can't represent at all). Every primitive/array/vec/option case below is
 * identical in both modes — Rust's own on-chain representation of a bare
 * primitive doesn't depend on Borsh vs zero-copy either.
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
        return getBooleanCodec() as unknown as NaclacCodec;
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
