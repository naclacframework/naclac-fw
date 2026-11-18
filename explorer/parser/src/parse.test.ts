/**
 * Tests of the pure parse step against recorded transactions: the mainnet pair
 * (raw result and Helius reference) and every devnet transaction recorded in
 * fixtures/devnet/raw. Run with `node --test src/parse.test.ts`.
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { ParseError } from "./errors.ts";
import { parseRawTransaction } from "./parse.ts";
import type { RpcTransactionResult, TransactionVersion } from "./types.ts";

const fixtures = path.resolve(import.meta.dirname, "../../fixtures");
const rawDir = path.join(fixtures, "devnet/raw");
const mainnetSignature =
  "61CGEEKgqxqfnSimYajSkTN3oLuFNtexxrAYFmBnCdZgqPd4XqthjU15jmBguAoKwX9h18YBDckFkooPzY4hz2V4";

interface IndexEntry {
  program: string;
  label: string;
  intent: string;
  requestedVersion: string;
  outcome: string;
}

const expectedVersion: Record<string, TransactionVersion> = { v1: 1, v0: 0, legacy: "legacy" };

function readJson(file: string): any {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function readRaw(file: string): RpcTransactionResult {
  return readJson(file).result;
}

const index: Record<string, IndexEntry> = readJson(path.join(rawDir, "index.json"));
const mainnetRaw = (): RpcTransactionResult =>
  readRaw(path.join(fixtures, `${mainnetSignature}.rpc.json`));
const devnetRaw = (signature: string): RpcTransactionResult =>
  readRaw(path.join(rawDir, `${signature}.rpc.json`));

function findEntry(program: string, label: string, requestedVersion: string): string {
  const found = Object.entries(index).find(
    ([, e]) => e.program === program && e.label === label && e.requestedVersion === requestedVersion,
  );
  assert.ok(found, `no fixture for ${program} / ${label} / ${requestedVersion}`);
  return found[0];
}

function assertParseError(action: () => unknown, field: string): void {
  assert.throws(
    action,
    (error: unknown) => error instanceof ParseError && error.field === field,
    `expected a ParseError for ${field}`,
  );
}

function at<T>(items: readonly T[], position: number): T {
  const item = items[position];
  assert.ok(item !== undefined, `no item at position ${position}`);
  return item;
}

function changeSum(changes: { change: string }[]): bigint {
  return changes.reduce((sum, c) => sum + BigInt(c.change), 0n);
}

test("mainnet v0: the flat instruction list equals the reference fixture", () => {
  const reference = readJson(path.join(fixtures, `${mainnetSignature}.json`));
  const ours = parseRawTransaction(mainnetRaw());
  const project = (i: any) => ({
    instructionIndex: i.instructionIndex,
    innerInstructionIndex: i.innerInstructionIndex,
    stackHeight: i.stackHeight,
    programId: i.programId,
    rawAccounts: i.rawAccounts,
    rawData: i.rawData,
  });
  assert.equal(ours.parsed.instructions.length, 26);
  assert.deepEqual(
    ours.parsed.instructions.map(project),
    reference.parsed.instructions.map(project),
  );
});

test("mainnet v0: header fields equal the reference fixture", () => {
  const reference = readJson(path.join(fixtures, `${mainnetSignature}.json`)).parsed;
  const { signature, parsed } = parseRawTransaction(mainnetRaw());
  assert.equal(signature, mainnetSignature);
  assert.equal(parsed.slot, reference.slot);
  assert.equal(parsed.blockTime, reference.blockTime);
  assert.equal(parsed.fee, reference.fee);
  assert.equal(parsed.feePayer, reference.feePayer);
  assert.equal(parsed.transactionStatus, reference.transactionStatus);
  assert.equal(parsed.version, 0);
});

test("mainnet v0: account keys are static, then lookup writable, then lookup readonly", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const bySource = (source: string) => parsed.accountKeys.filter((k) => k.source === source);
  assert.equal(parsed.accountKeys.length, 49);
  assert.equal(bySource("static").length, 14);
  assert.equal(bySource("lookup-writable").length, 18);
  assert.equal(bySource("lookup-readonly").length, 17);
  assert.deepEqual(
    parsed.accountKeys.map((k) => k.index),
    Array.from({ length: 49 }, (_, i) => i),
  );
  assert.ok(bySource("lookup-writable").every((k) => k.writable && !k.signer));
  assert.ok(bySource("lookup-readonly").every((k) => !k.writable && !k.signer));
});

test("every devnet fixture parses and satisfies the structural invariants", () => {
  const signatures = Object.keys(index);
  assert.equal(signatures.length, 65);
  for (const signature of signatures) {
    const entry = index[signature];
    assert.ok(entry, signature);
    const raw = devnetRaw(signature);
    const { parsed } = parseRawTransaction(raw);
    const where = `${entry.program} / ${entry.label} / ${signature}`;

    assert.equal(parsed.version, expectedVersion[entry.requestedVersion], where);
    assert.equal(
      parsed.transactionStatus,
      entry.outcome === "succeeded" ? "OK" : "ERROR",
      where,
    );
    assert.equal(parsed.transactionConfig !== null, parsed.version === 1, where);
    assert.equal(parsed.logsTruncated, false, where);
    assert.equal(parsed.innerInstructionsRecorded, true, where);
    assert.equal(typeof parsed.computeUnitsConsumed, "number", where);
    assert.equal(typeof parsed.costUnits, "number", where);

    const topLevel = parsed.instructions.filter((i) => i.innerInstructionIndex === null);
    assert.equal(topLevel.length, raw.transaction.message.instructions.length, where);
    assert.deepEqual(
      topLevel.map((i) => i.instructionIndex),
      topLevel.map((_, i) => i),
      where,
    );
    assert.equal(parsed.feePayer, raw.transaction.message.accountKeys[0], where);
    const payer = at(parsed.accountKeys, 0);
    assert.ok(payer.signer && payer.writable, where);
    assert.equal(changeSum(parsed.balanceChanges), -BigInt(parsed.fee), where);
  }
});

test("a transaction with two signers: signer and writable flags follow the header", () => {
  const signature = findEntry("counter_borsh", "increment with the wrong authority", "v1");
  const { parsed } = parseRawTransaction(devnetRaw(signature));
  assert.deepEqual(
    parsed.accountKeys.map((k) => [k.signer, k.writable]),
    [
      [true, true],
      [true, true],
      [false, true],
      [false, false],
    ],
  );
  assert.equal(parsed.fee, 10000);
  assert.deepEqual(parsed.error, { InstructionError: [0, { Custom: 6000 }] });
  const instruction = at(parsed.instructions, 0);
  assert.equal(instruction.programId, at(parsed.accountKeys, 3).address);
  assert.deepEqual(instruction.rawAccounts, [
    at(parsed.accountKeys, 1).address,
    at(parsed.accountKeys, 2).address,
  ]);
  assert.equal(instruction.stackHeight, 1);
});

test("v1 transactions carry their compute budget in transactionConfig", () => {
  const signature = findEntry("counter_borsh", "increment", "v1");
  const { parsed } = parseRawTransaction(devnetRaw(signature));
  assert.deepEqual(parsed.transactionConfig, {
    computeUnitLimit: 1400000,
    heapSize: null,
    loadedAccountsDataSizeLimit: 67108864,
    priorityFee: null,
  });
});

test("malformed input is rejected with the offending field", () => {
  const v1 = findEntry("counter_borsh", "increment", "v1");

  let raw = structuredClone(devnetRaw(v1));
  at(raw.transaction.message.instructions, 0).programIdIndex = 99;
  assertParseError(() => parseRawTransaction(raw), "message.instructions[0].programIdIndex");

  raw = structuredClone(devnetRaw(v1));
  raw.transaction.message.header.numRequiredSignatures = 0;
  assertParseError(() => parseRawTransaction(raw), "message.header.numRequiredSignatures");

  raw = structuredClone(devnetRaw(v1));
  raw.meta = null;
  assertParseError(() => parseRawTransaction(raw), "meta");

  raw = structuredClone(devnetRaw(v1));
  (raw as { version: unknown }).version = 2;
  assertParseError(() => parseRawTransaction(raw), "version");

  raw = structuredClone(devnetRaw(v1));
  delete raw.version;
  assertParseError(() => parseRawTransaction(raw), "version");

  raw = structuredClone(devnetRaw(v1));
  raw.meta!.preBalances.pop();
  assertParseError(() => parseRawTransaction(raw), "meta.preBalances");

  raw = structuredClone(mainnetRaw());
  delete raw.meta!.loadedAddresses;
  assertParseError(() => parseRawTransaction(raw), "meta.loadedAddresses");

  raw = structuredClone(mainnetRaw());
  raw.meta!.innerInstructions!.push(structuredClone(at(raw.meta!.innerInstructions!, 0)));
  assertParseError(() => parseRawTransaction(raw), "meta.innerInstructions");

  raw = structuredClone(mainnetRaw());
  at(raw.meta!.innerInstructions!, 0).index = 50;
  assertParseError(() => parseRawTransaction(raw), "meta.innerInstructions");
});

test("a node that omits inner instructions is reported, not read as none", () => {
  const raw = structuredClone(devnetRaw(findEntry("counter_borsh", "increment", "v1")));
  raw.meta!.innerInstructions = null;
  const { parsed } = parseRawTransaction(raw);
  assert.equal(parsed.innerInstructionsRecorded, false);
});
