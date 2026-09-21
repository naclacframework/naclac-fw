import type { IdlInstruction } from "../idl";
import { getIdlCodec, getStructFieldsCodec } from "./types";
import { toCamelCase } from "../utils/case";

/**
 * Serializes the arguments for a single instruction into a Uint8Array.
 *
 * Layout: [ discriminator (8 bytes) | arg0 | arg1 | ... ]
 *
 * The 8-byte discriminator is read directly from the IDL (pre-computed by
 * build.rs using sha256("global:<name>")[..8]) — no runtime hashing needed.
 *
 * @param instruction - The full IDL instruction object.
 * @param args        - A key-value map of argument names to their values.
 * @returns           A Uint8Array ready to be used as instruction data.
 */
export function encodeInstructionData(
  instruction: IdlInstruction,
  args: Record<string, unknown>,
  definedTypes?: readonly any[],
  isZeroCopy = false
): Uint8Array {
  const discriminator = new Uint8Array(instruction.discriminator);

  if (instruction.args.length === 0) {
    return discriminator;
  }

  const encodedArgs: Uint8Array[] = instruction.args.map((argDef) => {
    const camelName = toCamelCase(argDef.name);
    const value = args[camelName];
    if (value === undefined) {
      throw new Error(
        `[Naclac] Missing argument "${camelName}" for instruction "${instruction.name}". ` +
        `Expected arguments: [${instruction.args.map((a) => toCamelCase(a.name)).join(", ")}].`
      );
    }
    const codec = getIdlCodec(argDef.type, definedTypes, isZeroCopy);
    return codec.encode(value);
  });

  const totalLength =
    discriminator.length +
    encodedArgs.reduce((sum, buf) => sum + buf.length, 0);

  const data = new Uint8Array(totalLength);
  let offset = 0;

  data.set(discriminator, offset);
  offset += discriminator.length;

  for (const encoded of encodedArgs) {
    data.set(encoded, offset);
    offset += encoded.length;
  }

  return data;
}

/**
 * Decodes raw on-chain account bytes for a named account type.
 * Skips the leading 8-byte discriminator and deserializes field by field.
 *
 * Uses `getStructFieldsCodec` rather than a naive sequential per-field walk
 * — a zero-copy account's own top-level fields can have real internal
 * alignment gaps (`#[repr(C)]`, closed on-chain by `pod_struct_checks`'s
 * auto-inserted padding, which never reaches the IDL — see
 * `getStructFieldsCodec`'s own doc comment) that a naive walk has no way to
 * know about. Borsh mode never has this concern, so it stays sequential
 * either way.
 *
 * @param fields - The IDL field definitions for the account type.
 * @param data   - Raw bytes from the RPC (the full account data, excluding length prefix).
 * @returns      A plain object with decoded field values.
 */
export function decodeAccountData(
  fields: readonly { name: string; type: string | Record<string, unknown> }[],
  data: Uint8Array,
  definedTypes?: readonly any[],
  isZeroCopy = false
): Record<string, unknown> {
  const codec = getStructFieldsCodec(fields as any[], definedTypes, isZeroCopy);
  const [value] = codec.read(data, 8);
  return value as Record<string, unknown>;
}
