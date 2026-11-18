/**
 * Tests of IDL resolution from a metadata account: every outcome, each reason
 * for `invalid`, compression formats, and the decompression size limit, using
 * accounts built in the test. Run with `node --test src/resolve.test.ts`.
 */

import assert from "node:assert/strict";
import { gzipSync, deflateSync } from "node:zlib";
import { test } from "node:test";
import { decodeBase58 } from "./base58.ts";
import {
  COMPRESSION,
  DATA_SOURCE,
  ENCODING,
  FORMAT,
  HEADER_LEN,
  MAX_IDL_BYTES,
  PROGRAM_METADATA_ID,
  idlMetadataAddress,
  parseHeader,
} from "./pmp.ts";
import { resolveIdl } from "./resolve.ts";
import type { AccountData } from "./resolve.ts";
import { counterLocalIdls, plain, toOnchain } from "./test-support.ts";

const local = counterLocalIdls().counter_borsh;
const programId: string = local.address;

interface AccountOptions {
  kind?: number;
  program?: string;
  authority?: string | null;
  mutable?: boolean;
  canonical?: boolean;
  encoding?: number;
  compression?: number;
  format?: number;
  dataSource?: number;
  declaredLength?: number;
  owner?: string;
}

function account(payload: Uint8Array, options: AccountOptions = {}): AccountData {
  const data = new Uint8Array(HEADER_LEN + payload.length);
  data[0] = options.kind ?? 2;
  data.set(decodeBase58(options.program ?? programId) as Uint8Array, 1);
  if (options.authority) data.set(decodeBase58(options.authority) as Uint8Array, 33);
  data[65] = options.mutable === false ? 0 : 1;
  data[66] = options.canonical === false ? 0 : 1;
  data.set(new TextEncoder().encode("idl"), 67);
  data[83] = options.encoding ?? ENCODING.UTF8;
  data[84] = options.compression ?? COMPRESSION.ZLIB;
  data[85] = options.format ?? FORMAT.JSON;
  data[86] = options.dataSource ?? DATA_SOURCE.DIRECT;
  new DataView(data.buffer).setUint32(87, options.declaredLength ?? payload.length, true);
  data.set(payload, HEADER_LEN);
  return { owner: options.owner ?? PROGRAM_METADATA_ID, data };
}

const json = (value: unknown): Buffer => Buffer.from(JSON.stringify(value));
const onchainJson = json(toOnchain(local));

test("a zlib-compressed naclac IDL resolves to the local IDL and the metadata details", async () => {
  const authority = "28ArNHwi2Fg4WLfzdqWXrdaXg7hVZ1zam1bpd23hybWJ";
  const result = await resolveIdl(programId, account(deflateSync(onchainJson), { authority }));
  assert.equal(result.kind, "naclac");
  if (result.kind !== "naclac") return;
  assert.deepEqual(plain(result.document), local);
  assert.equal(result.authority, authority);
  assert.equal(result.mutable, true);
  assert.equal(result.metadataAddress, await idlMetadataAddress(programId));
});

test("gzip and uncompressed payloads resolve the same way", async () => {
  for (const [compression, payload] of [
    [COMPRESSION.GZIP, gzipSync(onchainJson)],
    [COMPRESSION.NONE, onchainJson],
  ] as const) {
    const result = await resolveIdl(programId, account(payload, { compression }));
    assert.equal(result.kind, "naclac", `compression ${compression}`);
    if (result.kind === "naclac") assert.deepEqual(plain(result.document), local);
  }
});

test("an account with no authority recorded resolves with authority null", async () => {
  const result = await resolveIdl(programId, account(deflateSync(onchainJson)));
  assert.equal(result.kind === "naclac" && result.authority, null);
});

test("an IDL that naclac did not write is reported as unsupported, not converted", async () => {
  const anchorLike = json({ address: programId, metadata: { name: "x", spec: "0.1.0" }, instructions: [] });
  const result = await resolveIdl(programId, account(deflateSync(anchorLike)));
  assert.equal(result.kind, "unsupported-idl");
});

test("nothing published, an empty account and an unfinished buffer all resolve to none", async () => {
  assert.equal((await resolveIdl(programId, null)).kind, "none");
  assert.equal((await resolveIdl(programId, account(new Uint8Array(), { kind: 0 }))).kind, "none");
  const buffer = await resolveIdl(programId, account(deflateSync(onchainJson), { kind: 1 }));
  assert.equal(buffer.kind, "none");
  assert.match(buffer.reason, /unfinished upload buffer/);
});

test("each way a stored account can be untrustworthy resolves to invalid with its reason", async () => {
  const good = deflateSync(onchainJson);
  const cases: [string, AccountData, RegExp][] = [
    ["foreign owner", account(good, { owner: "11111111111111111111111111111111" }), /not the Program Metadata Program/],
    ["other program", account(good, { program: PROGRAM_METADATA_ID }), /belongs to program/],
    ["not canonical", account(good, { canonical: false }), /not marked canonical/],
    ["by reference", account(good, { dataSource: DATA_SOURCE.URL }), /stored by reference/],
    ["not utf-8", account(good, { encoding: ENCODING.BASE64 }), /only UTF-8 JSON/],
    ["not json", account(good, { format: FORMAT.YAML }), /only UTF-8 JSON/],
    ["unknown compression", account(good, { compression: 9 }), /unrecognized compression byte 9/],
    ["truncated", account(good, { declaredLength: good.length + 10 }), /truncated/],
    ["short header", { owner: PROGRAM_METADATA_ID, data: new Uint8Array(10) }, /shorter than 96 bytes/],
    ["bad discriminator", account(good, { kind: 7 }), /unrecognised/],
    ["corrupt zlib", account(Buffer.from("not zlib data")), /could not be decompressed/],
    ["not json text", account(deflateSync(Buffer.from("{nope"))), /not valid UTF-8 JSON/],
    ["not utf-8 bytes", account(deflateSync(Buffer.from([0xff, 0xfe, 0xfd]))), /not valid UTF-8 JSON/],
    ["missing types", account(deflateSync(json({ ...toOnchain(local), types: undefined }))), /no `types` list/],
  ];
  for (const [name, input, reason] of cases) {
    const result = await resolveIdl(programId, input);
    assert.equal(result.kind, "invalid", name);
    assert.match((result as { reason: string }).reason, reason, name);
  }
});

test("a payload that decompresses past the size limit is rejected, not buffered", async () => {
  const bomb = deflateSync(Buffer.alloc(MAX_IDL_BYTES + 1));
  assert.ok(bomb.length < 100_000);
  const result = await resolveIdl(programId, account(bomb));
  assert.equal(result.kind, "invalid");
  assert.match((result as { reason: string }).reason, /decompresses to more than/);

  const justUnder = deflateSync(Buffer.concat([onchainJson, Buffer.alloc(0)]));
  assert.equal((await resolveIdl(programId, account(justUnder))).kind, "naclac");
});

test("header fields are read at the layout's offsets", () => {
  const input = account(new Uint8Array([1, 2, 3]), {
    authority: "28ArNHwi2Fg4WLfzdqWXrdaXg7hVZ1zam1bpd23hybWJ",
    mutable: false,
    compression: COMPRESSION.GZIP,
  });
  const header = parseHeader(input.data);
  assert.ok(header);
  assert.equal(header.kind, "metadata");
  assert.equal(header.program, programId);
  assert.equal(header.authority, "28ArNHwi2Fg4WLfzdqWXrdaXg7hVZ1zam1bpd23hybWJ");
  assert.equal(header.mutable, false);
  assert.equal(header.canonical, true);
  assert.equal(new TextDecoder().decode(header.seed.subarray(0, 3)), "idl");
  assert.equal(header.compression, COMPRESSION.GZIP);
  assert.equal(header.dataLength, 3);
});

test("a malformed program id is refused before anything is derived", async () => {
  await assert.rejects(resolveIdl("not-an-address", null), /not a valid 32-byte address/);
  await assert.rejects(idlMetadataAddress("1111"), /not a valid 32-byte address/);
});
