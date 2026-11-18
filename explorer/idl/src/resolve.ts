/**
 * Turns the account at a program's canonical IDL metadata address into one of
 * four explicit outcomes. The caller fetches the account (its address comes
 * from `idlMetadataAddress`); this module only reads it.
 */

import { IdlError } from "./errors.ts";
import { isNaclacIdl, toLocalIdl } from "./convert.ts";
import {
  COMPRESSION,
  DATA_SOURCE,
  ENCODING,
  FORMAT,
  PROGRAM_METADATA_ID,
  decompress,
  idlMetadataAddress,
  parseHeader,
  payloadOf,
} from "./pmp.ts";

export interface AccountData {
  owner: string;
  data: Uint8Array;
}

export interface MetadataInfo {
  metadataAddress: string;
  authority: string | null;
  mutable: boolean;
}

/**
 * `naclac`: a naclac IDL, converted to the local form (`document`, still
 * unvalidated; the parser validates it). `unsupported-idl`: a readable IDL that
 * is not a naclac one. `none`: nothing usable was ever published. `invalid`:
 * something is stored but cannot be trusted or read, with the reason.
 */
export type IdlResolution =
  | ({ kind: "naclac"; document: unknown } & MetadataInfo)
  | ({ kind: "unsupported-idl"; reason: string } & MetadataInfo)
  | { kind: "none"; reason: string; metadataAddress: string }
  | { kind: "invalid"; reason: string; metadataAddress: string };

export { idlMetadataAddress };

/**
 * Resolves the IDL of `programId` from `account`, the data at its canonical
 * metadata address (`null` when the cluster has no such account).
 */
export async function resolveIdl(
  programId: string,
  account: AccountData | null,
): Promise<IdlResolution> {
  const metadataAddress = await idlMetadataAddress(programId);
  const invalid = (reason: string): IdlResolution => ({ kind: "invalid", reason, metadataAddress });
  const none = (reason: string): IdlResolution => ({ kind: "none", reason, metadataAddress });

  if (account === null) return none("no metadata account exists at the program's IDL address.");
  if (account.owner !== PROGRAM_METADATA_ID) {
    return invalid(`the account is owned by ${account.owner}, not the Program Metadata Program.`);
  }
  const header = parseHeader(account.data);
  if (header === null) return invalid("the account's header is shorter than 96 bytes or unrecognised.");
  if (header.kind === "empty") return none("the metadata account is empty.");
  if (header.kind === "buffer") return none("the metadata account holds an unfinished upload buffer.");
  if (header.program !== programId) {
    return invalid(`the metadata account belongs to program ${header.program}, not ${programId}.`);
  }
  if (!header.canonical) return invalid("the account at the canonical address is not marked canonical.");
  if (header.dataSource !== DATA_SOURCE.DIRECT) {
    return invalid(`the IDL is stored by reference (data source ${header.dataSource}), which is not followed.`);
  }
  if (header.encoding !== ENCODING.UTF8 || header.format !== FORMAT.JSON) {
    return invalid(`the IDL uses encoding ${header.encoding} and format ${header.format}; only UTF-8 JSON is read.`);
  }
  if (header.compression !== COMPRESSION.NONE && header.compression !== COMPRESSION.GZIP && header.compression !== COMPRESSION.ZLIB) {
    return invalid(`unrecognized compression byte ${header.compression}.`);
  }
  const payload = payloadOf(account.data, header);
  if (payload === null) return invalid("the metadata account is truncated.");

  let document: unknown;
  try {
    const bytes = await decompress(header.compression, payload);
    document = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch (error) {
    return invalid(error instanceof IdlError ? error.message : "the IDL is not valid UTF-8 JSON.");
  }

  const info: MetadataInfo = { metadataAddress, authority: header.authority, mutable: header.mutable };
  if (!isNaclacIdl(document)) {
    return { kind: "unsupported-idl", reason: "the IDL was not written by naclac.", ...info };
  }
  try {
    return { kind: "naclac", document: toLocalIdl(document), ...info };
  } catch (error) {
    return invalid(error instanceof IdlError ? error.message : "the IDL could not be converted.");
  }
}
