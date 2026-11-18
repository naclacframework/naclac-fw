/**
 * Unit tests of the parser's building blocks: base58, field layout decoding,
 * the log walker, the framework error table and IDL validation. Run with
 * `node --test src/units.test.ts`.
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { decodeBase58, encodeBase58 } from "./base58.ts";
import { ParseError } from "./errors.ts";
import { FRAMEWORK_ERROR_BASE, FRAMEWORK_ERRORS, USER_ERROR_BASE } from "./framework-errors.ts";
import { createIdlMap, parseNaclacIdl } from "./idl.ts";
import { decodeFields } from "./layout.ts";
import { walkLogs } from "./logs.ts";
import { counterIdlDocuments, devnetRaw, index, mainnetRaw } from "./test-support.ts";

function assertParseError(action: () => unknown, field: string): void {
  assert.throws(
    action,
    (error: unknown) => error instanceof ParseError && error.field === field,
    `expected a ParseError for ${field}`,
  );
}

test("base58: known vectors", () => {
  assert.equal(encodeBase58(new Uint8Array(32)), "11111111111111111111111111111111");
  assert.deepEqual(decodeBase58("11111111111111111111111111111111", "x"), new Uint8Array(32));
  assert.equal(encodeBase58(new Uint8Array([0, 0, 1])), "112");
  assert.deepEqual(decodeBase58("112", "x"), new Uint8Array([0, 0, 1]));
  assert.equal(encodeBase58(new Uint8Array()), "");
  assert.deepEqual(decodeBase58("", "x"), new Uint8Array());
  assert.equal(encodeBase58(new TextEncoder().encode("hello")), "Cn8eVZg");
});

test("base58: rejects characters outside the alphabet", () => {
  for (const bad of ["0", "O", "I", "l", "ab c", "é"]) {
    assertParseError(() => decodeBase58(bad, "data"), "data");
  }
});

test("base58: every address of every fixture round-trips and is 32 bytes", () => {
  const raws = [mainnetRaw(), ...Object.keys(index).map((signature) => devnetRaw(signature))];
  for (const raw of raws) {
    const addresses = [
      ...raw.transaction.message.accountKeys,
      ...(raw.meta?.loadedAddresses?.writable ?? []),
      ...(raw.meta?.loadedAddresses?.readonly ?? []),
    ];
    for (const address of addresses) {
      const bytes = decodeBase58(address, "address");
      assert.equal(bytes.length, 32, address);
      assert.equal(encodeBase58(bytes), address);
    }
  }
});

test("layout: fixed-width integers are little-endian and wide ones are decimal strings", () => {
  const bytes = new Uint8Array([
    0xff, 0x34, 0x12, 0x04, 0x03, 0x02, 0x01, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
  ]);
  const fields = [
    { name: "a", type: "u8" },
    { name: "b", type: "u16" },
    { name: "c", type: "u32" },
    { name: "d", type: "i64" },
  ];
  const result = decodeFields(fields, bytes, 0, "borsh");
  assert.ok(result.ok);
  assert.equal(result.values.a, 255);
  assert.equal(result.values.b, 0x1234);
  assert.equal(result.values.c, 0x01020304);
  assert.equal(result.values.d, "-2");
  assert.equal(result.end, 15);
});

test("layout: 128-bit integers and a public key", () => {
  const all = new Uint8Array(16).fill(0xff);
  const unsigned = decodeFields([{ name: "x", type: "u128" }], all, 0, "wincode");
  assert.ok(unsigned.ok);
  assert.equal(unsigned.values.x, "340282366920938463463374607431768211455");
  const signed = decodeFields([{ name: "x", type: "i128" }], all, 0, "wincode");
  assert.ok(signed.ok);
  assert.equal(signed.values.x, "-1");
  const key = decodeFields([{ name: "k", type: "publicKey" }], new Uint8Array(32), 0, "borsh");
  assert.ok(key.ok);
  assert.equal(key.values.k, "11111111111111111111111111111111");
});

test("layout: a field list starts after the discriminator", () => {
  const bytes = new Uint8Array([9, 9, 9, 9, 7, 0, 0, 0, 0, 0, 0, 0]);
  const result = decodeFields([{ name: "n", type: "u64" }], bytes, 4, "bytemuck");
  assert.ok(result.ok);
  assert.equal(result.values.n, "7");
  assert.equal(result.end, 12);
});

test("layout: bool is strict in borsh and wincode, lenient in bytemuck", () => {
  const field = [{ name: "flag", type: "bool" }];
  const two = new Uint8Array([2]);
  assert.equal(decodeFields(field, two, 0, "borsh").ok, false);
  assert.equal(decodeFields(field, two, 0, "wincode").ok, false);
  const lenient = decodeFields(field, two, 0, "bytemuck");
  assert.ok(lenient.ok);
  assert.equal(lenient.values.flag, true);
  const one = decodeFields(field, new Uint8Array([1]), 0, "borsh");
  assert.ok(one.ok);
  assert.equal(one.values.flag, true);
});

test("layout: short data is a decode error, other types are unsupported", () => {
  const short = decodeFields([{ name: "n", type: "u64" }], new Uint8Array(3), 0, "borsh");
  assert.ok(!short.ok);
  assert.equal(short.status, "decode-error");
  for (const type of ["string", { vec: "u8" }, { option: "u8" }, { defined: "T" }, "bytes", "nope"]) {
    const result = decodeFields([{ name: "x", type }], new Uint8Array(64), 0, "borsh");
    assert.ok(!result.ok);
    assert.equal(result.status, "unsupported-type");
  }
});

test("layout: a field named like an object property cannot change the result's prototype", () => {
  const fields = [
    { name: "__proto__", type: "u8" },
    { name: "constructor", type: "u8" },
  ];
  const result = decodeFields(fields, new Uint8Array([5, 6]), 0, "borsh");
  assert.ok(result.ok);
  assert.equal(Object.getPrototypeOf(result.values), null);
  assert.deepEqual(Object.keys(result.values), ["__proto__", "constructor"]);
  assert.equal(result.values["__proto__"], 5);
  const looksLikeAType = decodeFields([{ name: "x", type: "constructor" }], new Uint8Array(8), 0, "borsh");
  assert.ok(!looksLikeAType.ok);
  assert.equal(looksLikeAType.status, "unsupported-type");
});

test("logs: events belong to the innermost running program, and the first failure is kept", () => {
  const walked = walkLogs([
    "Program A111 invoke [1]",
    "Program data: AQID",
    "Program B222 invoke [2]",
    "Program data: BAUG BwgJ",
    "Program log: hello",
    "Program B222 failed: custom program error: 0x1771",
    "Program A111 failed: custom program error: 0x1771",
  ]);
  assert.deepEqual(walked.invocations, [
    { programId: "A111", depth: 1 },
    { programId: "B222", depth: 2 },
  ]);
  assert.deepEqual(walked.data, [
    { invocation: 0, programId: "A111", chunks: ["AQID"] },
    { invocation: 1, programId: "B222", chunks: ["BAUG", "BwgJ"] },
  ]);
  assert.deepEqual(walked.failure, {
    programId: "B222",
    reason: "custom program error: 0x1771",
  });
});

test("logs: inconsistent nesting is rejected", () => {
  assertParseError(() => walkLogs(["Program A111 invoke [2]"]), "meta.logMessages");
  assertParseError(
    () => walkLogs(["Program A111 invoke [1]", "Program B222 success"]),
    "meta.logMessages",
  );
  assertParseError(() => walkLogs(["Program data: AQID"]), "meta.logMessages");
});

test("framework errors: the table matches NaclacError in core/src/error.rs", () => {
  const source = fs.readFileSync(
    path.resolve(import.meta.dirname, "../../../core/src/error.rs"),
    "utf8",
  );
  const start = source.indexOf("pub enum NaclacError");
  const body = source.slice(start, source.indexOf("}", start));
  const entries = [...body.matchAll(/^\s+(\w+) = (\d+),$/gm)].map(
    (match): [number, string] => [Number(match[2]), match[1] as string],
  );
  assert.ok(entries.length > 30);
  assert.deepEqual(new Map(entries), FRAMEWORK_ERRORS);
  const constant = (name: string): number =>
    Number((new RegExp(`pub const ${name}: u32 = ([\\d_]+);`).exec(source)?.[1] ?? "").replace(/_/g, ""));
  assert.equal(constant("USER_ERROR_BASE"), USER_ERROR_BASE);
  assert.equal(constant("FRAMEWORK_ERROR_BASE"), FRAMEWORK_ERROR_BASE);
});

test("idl: all five counter IDLs validate", () => {
  const documents = counterIdlDocuments();
  assert.equal(Object.keys(documents).length, 5);
  for (const [name, document] of Object.entries(documents)) {
    const idl = parseNaclacIdl(document);
    assert.equal(idl.name, name);
    assert.deepEqual(
      idl.instructions.map((i) => i.name),
      ["initialize", "increment", "decrement"],
    );
    assert.deepEqual(
      idl.errors.map((e) => e.code),
      [6000, 6001],
    );
  }
  assert.equal(createIdlMap(Object.values(documents).map(parseNaclacIdl)).size, 5);
});

test("idl: malformed documents are rejected with the offending path", () => {
  const good = () => structuredClone(counterIdlDocuments().counter_borsh);

  let doc = good();
  doc.metadata.serialization = "json";
  assertParseError(() => parseNaclacIdl(doc), "idl.metadata.serialization");

  doc = good();
  doc.address = "1111";
  assertParseError(() => parseNaclacIdl(doc), "idl.address");

  doc = good();
  doc.instructions[1].discriminator = doc.instructions[0].discriminator;
  assertParseError(() => parseNaclacIdl(doc), "idl.instructions[1].discriminator");

  doc = good();
  doc.instructions[1].discriminator = [1, 2, 3];
  assertParseError(() => parseNaclacIdl(doc), "idl.instructions[1].discriminator");

  doc = good();
  doc.instructions[0].discriminator = [];
  assertParseError(() => parseNaclacIdl(doc), "idl.instructions[0].discriminator");

  doc = good();
  doc.instructions[0].discriminator = [300, 1, 1, 1, 1, 1, 1, 1];
  assertParseError(() => parseNaclacIdl(doc), "idl.instructions[0].discriminator[0]");

  doc = good();
  doc.events[0].fields.push({ name: "new_count", type: "u64" });
  assertParseError(() => parseNaclacIdl(doc), "idl.events[0].fields[2].name");

  doc = good();
  doc.errors[0].code = -1;
  assertParseError(() => parseNaclacIdl(doc), "idl.errors[0].code");

  assertParseError(() => parseNaclacIdl([]), "idl.root");
  assertParseError(
    () => createIdlMap([parseNaclacIdl(good()), parseNaclacIdl(good())]),
    "idl.address",
  );
});
