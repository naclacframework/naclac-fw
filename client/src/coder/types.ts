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
      return { size: 1, alignment: 1 };
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
  isZeroCopy: boolean
): NaclacCodec {
  return isZeroCopy ? getStructCodec(fields, definedTypes) : getBorshStructCodec(fields, definedTypes);
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
 * Borsh-mode enum codec — a `u8` tag equal to the variant's *positional*
 * index (0, 1, 2, ... in declaration order), never its real Rust
 * discriminant. Confirmed against real `borsh-derive` source
 * (`enum_discriminant.rs`): its default `Discriminants::get` uses
 * `variant_idx`, only switching to the real discriminant when a crate opts
 * in via `#[borsh(use_discriminant = true)]` — `naclac-macros`'s Borsh
 * branch never does, so positional indexing is exactly what real on-chain
 * Borsh encoding for a `#[defined_type]` enum produces. This is also
 * exactly `getDiscriminatedUnionCodec`'s own default wire behavior (it
 * writes the variant's array position, using the discriminator value only
 * to look up that position) — variant array order below must therefore
 * match IDL declaration order, which it does (`enumDef.variants` is never
 * reordered).
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
  const discCodec = getDiscriminatedUnionCodec(variantEntries, { discriminator: "kind" });
  return discCodec as unknown as NaclacCodec;
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
 * `naclac-macros/src/lib.rs`), then each variant's own fields laid out
 * exactly like a struct whose first field is the tag (the same trick
 * `naclac-macros/src/checked_enum.rs`'s payload structs use, and the same
 * reason it's sound — see that file's own comment citing the Rust
 * reference on primitive representation of enums with fields), and an
 * overall union size/alignment = the max over every variant's own
 * (already self-padded) size/alignment, itself rounded up to that overall
 * alignment (standard `#[repr(C)]` union sizing). This mirrors
 * `naclac-pod-codegen`'s — the Rust client's copy of `naclac-macros/src/
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
 * Zero-copy-mode enum codec — real C-layout (tag + per-variant fields with
 * alignment padding, see `computeEnumCLayout`), and the tag byte(s) written/
 * matched are the variant's *real* discriminant (`variant.discriminant`),
 * not its positional index — the opposite convention from
 * `getBorshEnumCodec`, and correct for zero-copy mode specifically because
 * `naclac-macros/src/checked_enum.rs`'s `variant_discriminants` mirrors
 * Rust's own real compiler discriminant rule (explicit `= N` respected),
 * unlike Borsh's derive default.
 */
function getZeroCopyEnumCodec(enumDef: { variants: any[]; repr?: string }, definedTypes?: readonly any[]): NaclacCodec {
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
        throw new Error(
          `[Naclac] Invalid enum discriminant ${String(tagValue)} while decoding — no variant of this ` +
          `enum declares it. This indicates corrupted account/instruction data, not a client bug.`
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

      return getArrayCodec(getIdlCodec(innerType, definedTypes, isZeroCopy) as any, { size }) as unknown as NaclacCodec;
    }
    if ("vec" in type) {
      const innerType = type.vec;
      return addCodecSizePrefix(getArrayCodec(getIdlCodec(innerType, definedTypes, isZeroCopy) as any), getU32Codec()) as unknown as NaclacCodec;
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
            ? getZeroCopyEnumCodec(def.type, definedTypes)
            : getBorshEnumCodec(def.type, definedTypes);
        }
      }
      return getU8Codec() as unknown as NaclacCodec;
    }
  }

  throw new Error(`[Naclac] Unknown IDL type "${JSON.stringify(type)}". Supported types include primitives, array, vec, option.`);
}
