/**
 * Tests of base58 and program address derivation, checked against addresses the
 * chain itself produced: the counter PDAs of every recorded devnet transaction
 * and an associated token account from the mainnet fixture. Run with
 * `node --test src/pda.test.ts`.
 */

import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import { decodeBase58, encodeBase58 } from "./base58.ts";
import { IdlError } from "./errors.ts";
import { createProgramAddress, findProgramAddress, isOnCurve } from "./pda.ts";
import { fixtures, rawDir, readJson } from "./test-support.ts";

const mainnetSignature =
  "61CGEEKgqxqfnSimYajSkTN3oLuFNtexxrAYFmBnCdZgqPd4XqthjU15jmBguAoKwX9h18YBDckFkooPzY4hz2V4";

function key(address: string): Uint8Array {
  const bytes = decodeBase58(address);
  assert.ok(bytes && bytes.length === 32, address);
  return bytes;
}

function recorded(): { program: string; counter: string; payer: string }[] {
  const index = readJson(path.join(rawDir, "index.json"));
  return Object.keys(index)
    .filter((signature) => index[signature].label !== "increment with the wrong counter account")
    .map((signature) => {
    const result = readJson(path.join(rawDir, `${signature}.rpc.json`)).result;
    const keys: string[] = result.transaction.message.accountKeys;
    const instruction = result.transaction.message.instructions[0];
    return {
      program: keys[instruction.programIdIndex] as string,
      counter: keys[instruction.accounts[1]] as string,
      payer: keys[0] as string,
    };
  });
}

test("base58: known vectors and rejection of foreign characters", () => {
  assert.equal(encodeBase58(new Uint8Array(32)), "11111111111111111111111111111111");
  assert.deepEqual(decodeBase58("112"), new Uint8Array([0, 0, 1]));
  assert.equal(encodeBase58(new TextEncoder().encode("hello")), "Cn8eVZg");
  for (const bad of ["0", "O", "I", "l", "a b"]) assert.equal(decodeBase58(bad), null);
});

test("counter PDAs derived from the program id equal the addresses in the 60 recorded transactions that use the real counter account", async () => {
  const seed = new TextEncoder().encode("counter");
  const rows = recorded();
  assert.equal(rows.length, 60);
  assert.equal(new Set(rows.map((r) => r.program)).size, 5);
  for (const row of rows) {
    const { address } = await findProgramAddress([seed], key(row.program));
    assert.equal(address, row.counter, `program ${row.program}`);
  }
});

test("an associated token account from the mainnet fixture is derived exactly", async () => {
  const raw = readJson(path.join(fixtures, `${mainnetSignature}.rpc.json`)).result;
  assert.equal(raw.transaction.message.accountKeys.length > 0, true);
  const owner = key("EBgTpjGvic1PP4WFf4QYX6qXKQxVkhkQhtyeaVvpfLu4");
  const tokenProgram = key("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
  const mint = key("So11111111111111111111111111111111111111112");
  const { address } = await findProgramAddress(
    [owner, tokenProgram, mint],
    key("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
  );
  assert.equal(address, "HpSRJZok15uyhuM2wHgNjEEZm8fFyMF2RxLPctWXVLm1");
});

test("the bump is the highest one that gives an off-curve address", async () => {
  const seed = new TextEncoder().encode("counter");
  for (const row of recorded().slice(0, 5)) {
    const program = key(row.program);
    const { address, bump } = await findProgramAddress([seed], program);
    assert.equal(await createProgramAddress([seed, Uint8Array.of(bump)], program), address);
    for (let higher = 255; higher > bump; higher--) {
      assert.equal(await createProgramAddress([seed, Uint8Array.of(higher)], program), null);
    }
  }
});

test("wallet keys are on the curve and PDAs are not", () => {
  for (const row of recorded().slice(0, 10)) {
    assert.equal(isOnCurve(key(row.payer)), true);
    assert.equal(isOnCurve(key(row.counter)), false);
  }
});

test("seed and key sizes are enforced", async () => {
  const program = key("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S");
  await assert.rejects(findProgramAddress(new Array(17).fill(new Uint8Array(1)), program), IdlError);
  await assert.rejects(findProgramAddress([new Uint8Array(33)], program), IdlError);
  assert.throws(() => isOnCurve(new Uint8Array(31)), IdlError);
});
