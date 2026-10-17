import type { NaclacIdl } from "../idl";
import { resolveSlots } from "../slots";
import { getStructFieldsCodec } from "./types";

/**
 * Encoder and decoder for the fields of one account type, discriminator
 * excluded: `decode` and `read` start at the first field.
 */
export interface AccountCodec<T> {
  encode: (value: T) => Uint8Array;
  decode: (bytes: Uint8Array) => T;
  read: (bytes: Uint8Array, offset: number) => [T, number];
}

/**
 * The codec for the account named `accountName`, built by the same runtime
 * coder `program.account(...)` uses, in the program's own serialization (the
 * IDL's `metadata.serialization`): the packed zero-copy layout for
 * `bytemuck`, Borsh or Wincode otherwise.
 *
 * @throws If the IDL defines no such account.
 */
export function getAccountCodec<T = Record<string, unknown>>(
  idl: NaclacIdl,
  accountName: string
): AccountCodec<T> {
  const resolved = resolveSlots(idl);
  const account = resolved.accounts.find((a) => a.name === accountName);
  if (!account) {
    throw new Error(`[Naclac] The IDL does not define an account named "${accountName}".`);
  }
  const codec = getStructFieldsCodec(
    account.type.fields as any[],
    resolved.definedTypes,
    resolved.metadata.serialization === "bytemuck"
  );
  return codec as unknown as AccountCodec<T>;
}
