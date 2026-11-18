/**
 * The Program Metadata Program's account layout, as `naclac idl upload` writes
 * and `naclac idl fetch` reads it (idl/src/program_metadata): the program id,
 * the canonical metadata address of a program's IDL, the 96-byte header, and
 * payload decompression. Byte offsets and constants are those of
 * idl/src/program_metadata/{mod,header,instructions}.rs.
 */

import { decodeBase58, encodeBase58 } from "./base58.ts";
import { IdlError } from "./errors.ts";
import { findProgramAddress } from "./pda.ts";

export const PROGRAM_METADATA_ID = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
export const HEADER_LEN = 96;
export const SEED_LEN = 16;
export const IDL_SEED = "idl";
export const MAX_IDL_BYTES = 10 * 1024 * 1024;

export const ENCODING = { NONE: 0, UTF8: 1, BASE58: 2, BASE64: 3 } as const;
export const COMPRESSION = { NONE: 0, GZIP: 1, ZLIB: 2 } as const;
export const FORMAT = { NONE: 0, JSON: 1, YAML: 2, TOML: 3 } as const;
export const DATA_SOURCE = { DIRECT: 0, URL: 1, EXTERNAL: 2 } as const;

export type MetadataKind = "empty" | "buffer" | "metadata";

export interface MetadataHeader {
  kind: MetadataKind;
  program: string;
  authority: string | null;
  mutable: boolean;
  canonical: boolean;
  seed: Uint8Array;
  encoding: number;
  compression: number;
  format: number;
  dataSource: number;
  dataLength: number;
}

const KINDS: readonly MetadataKind[] = ["empty", "buffer", "metadata"];

/** Decodes a program id, throwing an IdlError unless it is 32 bytes of valid base58. */
export function programIdBytes(programId: string): Uint8Array {
  const bytes = decodeBase58(programId);
  if (bytes === null || bytes.length !== 32) {
    throw new IdlError("the program id is not a valid 32-byte address.");
  }
  return bytes;
}

/** The canonical metadata address of `programId`'s IDL: `[program id, "idl" padded to 16 bytes]` under the metadata program. */
export async function idlMetadataAddress(programId: string): Promise<string> {
  const seed = new Uint8Array(SEED_LEN);
  seed.set(new TextEncoder().encode(IDL_SEED));
  const { address } = await findProgramAddress(
    [programIdBytes(programId), seed],
    programIdBytes(PROGRAM_METADATA_ID),
  );
  return address;
}

/** Parses a metadata account's header, or returns null when the data is shorter than the header or its discriminator is not one of the three real values. */
export function parseHeader(data: Uint8Array): MetadataHeader | null {
  if (data.length < HEADER_LEN) return null;
  const kind = KINDS[data[0] as number];
  if (kind === undefined) return null;
  const authority = data.subarray(33, 65);
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  return {
    kind,
    program: encodeBase58(data.subarray(1, 33)),
    authority: authority.every((byte) => byte === 0) ? null : encodeBase58(authority),
    mutable: data[65] !== 0,
    canonical: data[66] !== 0,
    seed: data.slice(67, 83),
    encoding: data[83] as number,
    compression: data[84] as number,
    format: data[85] as number,
    dataSource: data[86] as number,
    dataLength: view.getUint32(87, true),
  };
}

/** The payload bytes after the header, or null when the account holds fewer than `header.dataLength` of them. */
export function payloadOf(data: Uint8Array, header: MetadataHeader): Uint8Array | null {
  const end = HEADER_LEN + header.dataLength;
  return end <= data.length ? data.subarray(HEADER_LEN, end) : null;
}

/**
 * Decompresses a payload stored with the given compression byte. The result is
 * never larger than `MAX_IDL_BYTES`. Throws an IdlError for an unknown
 * compression byte, corrupt data, or output over the limit.
 */
export async function decompress(compression: number, payload: Uint8Array): Promise<Uint8Array> {
  if (compression === COMPRESSION.NONE) {
    if (payload.length > MAX_IDL_BYTES) {
      throw new IdlError(`the IDL payload is larger than ${MAX_IDL_BYTES} bytes.`);
    }
    return payload;
  }
  const format = compression === COMPRESSION.GZIP ? "gzip" : compression === COMPRESSION.ZLIB ? "deflate" : null;
  if (format === null) throw new IdlError(`unrecognized compression byte ${compression}.`);

  const source = new ReadableStream<Uint8Array<ArrayBuffer>>({
    start(controller) {
      controller.enqueue(payload.slice());
      controller.close();
    },
  });
  const reader = source.pipeThrough(new DecompressionStream(format)).getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.length;
      if (total > MAX_IDL_BYTES) {
        await reader.cancel();
        throw new IdlError(`the IDL payload decompresses to more than ${MAX_IDL_BYTES} bytes.`);
      }
      chunks.push(value);
    }
  } catch (error) {
    if (error instanceof IdlError) throw error;
    throw new IdlError("the IDL payload could not be decompressed.");
  }
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result;
}
