/**
 * Tests of naclac decoding against the 65 recorded counter transactions on
 * devnet (all five programs, all three serializations, all three transaction
 * versions, successes and failures), plus hostile and incomplete inputs. Run
 * with `node --test src/naclac.test.ts`.
 */

import assert from "node:assert/strict";
import { test } from "node:test";
import { encodeBase58 } from "./base58.ts";
import { BUILTIN_PROGRAMS } from "./builtin/index.ts";
import { FRAMEWORK_ERROR_BASE } from "./framework-errors.ts";
import { createIdlMap, parseNaclacIdl } from "./idl.ts";
import { walkLogs } from "./logs.ts";
import { parseRawTransaction } from "./parse.ts";
import {
  at,
  counterIdlDocuments,
  counterIdls,
  devnetRaw,
  index,
  mainnetRaw,
} from "./test-support.ts";

interface ExpectedFailure {
  kind: "program" | "framework";
  name: string;
  accountIndex?: number;
  accountName?: string;
  frameworkType?: number;
}

const expectedFailures: Record<string, ExpectedFailure> = {
  "increment with the wrong authority": { kind: "program", name: "Unauthorized" },
  "decrement at zero": { kind: "program", name: "Underflow" },
  "initialize a second time": {
    kind: "framework",
    name: "AccountAlreadyInitialized",
    accountIndex: 1,
    accountName: "counter_account",
    frameworkType: 14,
  },
  "increment with the wrong counter account": {
    kind: "framework",
    name: "ConstraintOwner",
    accountIndex: 1,
    accountName: "counter_account",
    frameworkType: 4,
  },
  "increment with the authority repeated as an extra account": {
    kind: "framework",
    name: "ConstraintDuplicateMutableAccount",
    accountIndex: 0,
    accountName: "authority",
    frameworkType: 25,
  },
};

function instructionOf(label: string): string {
  for (const name of ["increment", "decrement", "initialize"]) {
    if (label.startsWith(name)) return name;
  }
  throw new Error(`unexpected fixture label ${label}`);
}

function extraAccountsOf(label: string): number {
  if (label === "increment with two extra accounts") return 2;
  if (label === "increment with the authority repeated as an extra account") return 1;
  return 0;
}

const declaredAccounts: Record<string, string[]> = {
  initialize: ["payer", "counter_account", "system_program"],
  increment: ["authority", "counter_account"],
  decrement: ["authority", "counter_account"],
};

test("every counter fixture decodes its instruction, accounts and arguments", () => {
  const idls = counterIdls();
  for (const [signature, entry] of Object.entries(index)) {
    const where = `${entry.program} / ${entry.label} / ${signature}`;
    const { parsed } = parseRawTransaction(devnetRaw(signature), { idls });
    const name = instructionOf(entry.label);
    const instruction = at(parsed.instructions, 0);
    assert.equal(parsed.instructions.length, 1, where);
    assert.equal(instruction.decodeStatus, "decoded", where);
    assert.equal(instruction.decodeReason, null, where);
    assert.equal(instruction.programName, entry.program, where);
    assert.equal(instruction.instructionName, name, where);
    const decoded = instruction.decoded;
    assert.ok(decoded, where);
    assert.deepEqual(Object.keys(decoded.args), [], where);
    assert.equal(decoded.trailingBytes, 0, where);
    assert.deepEqual(decoded.missingAccounts, [], where);
    const expectedNames = [
      ...(declaredAccounts[name] ?? []),
      ...Array.from({ length: extraAccountsOf(entry.label) }, (_, i) => `remainingAccount${i + 1}`),
    ];
    assert.deepEqual(
      decoded.accounts.map((a) => a.name),
      expectedNames,
      where,
    );
    assert.ok(decoded.accounts.every((a) => a.provided), where);
    assert.deepEqual(
      decoded.accounts.map((a) => a.pubkey),
      instruction.rawAccounts,
      where,
    );
    assert.deepEqual(
      parsed.idls,
      [{ programId: instruction.programId, name: entry.program, serialization: parsed.idls[0]?.serialization }],
      where,
    );
  }
});

test("signer and writable flags of decoded accounts are the transaction's", () => {
  const idls = counterIdls();
  for (const [signature, entry] of Object.entries(index)) {
    const where = `${entry.program} / ${entry.label}`;
    const { parsed } = parseRawTransaction(devnetRaw(signature), { idls });
    const accounts = at(parsed.instructions, 0).decoded?.accounts ?? [];
    const first = at(accounts, 0);
    assert.ok(first.isSigner && first.isWritable, where);
    const counter = at(accounts, 1);
    assert.ok(!counter.isSigner && counter.isWritable, where);
    if (instructionOf(entry.label) === "initialize") {
      const system = at(accounts, 2);
      assert.ok(!system.isSigner && !system.isWritable, where);
    }
  }
});

test("successful increments and decrements emit one decoded event stamped at or just before the block time", () => {
  const idls = counterIdls();
  let checked = 0;
  for (const [signature, entry] of Object.entries(index)) {
    const where = `${entry.program} / ${entry.label} / ${signature}`;
    const { parsed } = parseRawTransaction(devnetRaw(signature), { idls });
    assert.equal(parsed.eventsAvailable, true, where);
    assert.deepEqual(parsed.undecodedLogData, [], where);
    if (entry.outcome !== "succeeded") {
      assert.deepEqual(parsed.events, [], where);
      continue;
    }
    const name = instructionOf(entry.label);
    assert.equal(parsed.events.length, 1, where);
    const event = at(parsed.events, 0);
    assert.equal(event.name, name === "increment" ? "CounterIncremented" : "CounterDecremented", where);
    assert.equal(event.programName, entry.program, where);
    assert.equal(event.instructionIndex, 0, where);
    assert.equal(event.innerInstructionIndex, null, where);
    assert.equal(event.trailingBytes, 0, where);
    assert.deepEqual(Object.keys(event.data), ["new_count", "timestamp"], where);
    assert.ok(parsed.blockTime !== null, where);
    const drift = Number(event.data.timestamp) - parsed.blockTime;
    assert.ok(drift >= -2 && drift <= 0, `${where}: event timestamp is ${drift}s from blockTime`);
    assert.match(String(event.data.new_count), /^\d+$/, where);
    checked++;
  }
  assert.equal(checked, 40);
});

test("failed transactions resolve their error against the program's IDL", () => {
  const idls = counterIdls();
  let checked = 0;
  for (const [signature, entry] of Object.entries(index)) {
    const where = `${entry.program} / ${entry.label} / ${signature}`;
    const { parsed } = parseRawTransaction(devnetRaw(signature), { idls });
    if (entry.outcome === "succeeded") {
      assert.equal(parsed.decodedError, null, where);
      continue;
    }
    const expected = expectedFailures[entry.label];
    assert.ok(expected, `no expectation for ${where}`);
    const error = parsed.decodedError;
    assert.ok(error, where);
    assert.equal(error.kind, expected.kind, where);
    assert.equal(error.name, expected.name, where);
    assert.equal(error.instructionIndex, 0, where);
    assert.equal(error.programId, at(parsed.instructions, 0).programId, where);
    assert.equal(error.programName, entry.program, where);
    if (expected.kind === "program") {
      const declared = idls
        .get(error.programId as string)
        ?.errors.find((candidate) => candidate.name === expected.name);
      assert.equal(error.code, declared?.code, where);
      assert.equal(error.msg, declared?.msg, where);
      assert.equal(error.accountIndex, null, where);
    } else {
      assert.equal(
        error.code,
        FRAMEWORK_ERROR_BASE + (expected.accountIndex as number) * 100 + (expected.frameworkType as number),
        where,
      );
      assert.equal(error.accountIndex, expected.accountIndex, where);
      assert.equal(error.accountName, expected.accountName, where);
    }
    checked++;
  }
  assert.equal(checked, 25);
});

test("without IDLs, instructions are marked no-idl and event data stays undecoded", () => {
  const signature = Object.entries(index).find(
    ([, e]) => e.label === "increment" && e.outcome === "succeeded",
  )?.[0];
  assert.ok(signature);
  const { parsed } = parseRawTransaction(devnetRaw(signature));
  const instruction = at(parsed.instructions, 0);
  assert.equal(instruction.decodeStatus, "no-idl");
  assert.equal(instruction.decoded, null);
  assert.equal(instruction.programName, null);
  assert.deepEqual(parsed.events, []);
  assert.equal(parsed.undecodedLogData.length, 1);
  assert.equal(at(parsed.undecodedLogData, 0).reason, "no IDL was supplied for this program.");
  assert.deepEqual(parsed.idls, []);
});

test("without IDLs a failed transaction's error is reported as unrecognised, not guessed", () => {
  const signature = Object.entries(index).find(
    ([, e]) => e.label === "increment with the wrong authority",
  )?.[0];
  assert.ok(signature);
  const { parsed } = parseRawTransaction(devnetRaw(signature));
  const error = parsed.decodedError;
  assert.ok(error);
  assert.equal(error.kind, "unrecognised");
  assert.equal(error.name, null);
  assert.equal(error.code, 6000);
  assert.equal(error.programName, null);
});

test("mainnet: the k-th log invocation is the k-th instruction, and without IDLs only built-in programs decode", () => {
  const raw = mainnetRaw();
  const { parsed } = parseRawTransaction(raw);
  const walked = walkLogs(raw.meta?.logMessages ?? []);
  assert.equal(walked.invocations.length, parsed.instructions.length);
  walked.invocations.forEach((invocation, k) => {
    assert.equal(invocation.programId, at(parsed.instructions, k).programId, `invocation ${k}`);
  });
  for (const instruction of parsed.instructions) {
    assert.equal(
      instruction.decodeStatus === "no-idl",
      !BUILTIN_PROGRAMS.has(instruction.programId),
      `${instruction.instructionIndex}/${instruction.innerInstructionIndex} ${instruction.programId}`,
    );
  }
  assert.equal(parsed.decodedError, null);
  assert.deepEqual(parsed.events, []);
});

function incrementOf(program: string): { signature: string; instruction: number[] } {
  const found = Object.entries(index).find(
    ([, e]) => e.program === program && e.label === "increment" && e.outcome === "succeeded",
  );
  assert.ok(found);
  const idl = parseNaclacIdl(counterIdlDocuments()[program]);
  const definition = idl.instructions.find((i) => i.name === "increment");
  assert.ok(definition);
  return { signature: found[0], instruction: definition.discriminator };
}

test("instruction data that matches no instruction is reported as unknown", () => {
  const idls = counterIdls();
  const { signature } = incrementOf("counter_borsh");
  for (const data of [new Uint8Array(8), new Uint8Array(3), new Uint8Array()]) {
    const raw = structuredClone(devnetRaw(signature));
    at(raw.transaction.message.instructions, 0).data = encodeBase58(data);
    const instruction = at(parseRawTransaction(raw, { idls }).parsed.instructions, 0);
    assert.equal(instruction.decodeStatus, "unknown-instruction");
    assert.equal(instruction.instructionName, null);
    assert.equal(instruction.decoded, null);
    assert.equal(instruction.programName, "counter_borsh");
  }
});

test("bytes after the last argument are counted, not hidden", () => {
  const idls = counterIdls();
  const { signature, instruction } = incrementOf("counter_borsh");
  const raw = structuredClone(devnetRaw(signature));
  at(raw.transaction.message.instructions, 0).data = encodeBase58(
    new Uint8Array([...instruction, 1, 2, 3]),
  );
  const decoded = at(parseRawTransaction(raw, { idls }).parsed.instructions, 0);
  assert.equal(decoded.decodeStatus, "decoded");
  assert.equal(decoded.decoded?.trailingBytes, 3);
});

test("arguments the IDL declares but the data lacks, or of an unsupported type, are reported", () => {
  const { signature, instruction } = incrementOf("counter_borsh");
  const withArg = (type: unknown) => {
    const document = structuredClone(counterIdlDocuments().counter_borsh);
    document.instructions.find((i: any) => i.name === "increment").args = [{ name: "x", type }];
    return createIdlMap([parseNaclacIdl(document)]);
  };
  const raw = structuredClone(devnetRaw(signature));
  at(raw.transaction.message.instructions, 0).data = encodeBase58(new Uint8Array(instruction));

  const short = at(parseRawTransaction(raw, { idls: withArg("u64") }).parsed.instructions, 0);
  assert.equal(short.decodeStatus, "decode-error");
  assert.equal(short.instructionName, "increment");
  assert.equal(short.decoded, null);

  const unsupported = at(parseRawTransaction(raw, { idls: withArg("string") }).parsed.instructions, 0);
  assert.equal(unsupported.decodeStatus, "unsupported-type");
  assert.match(unsupported.decodeReason ?? "", /not decoded yet/);

  at(raw.transaction.message.instructions, 0).data = encodeBase58(
    new Uint8Array([...instruction, 42, 0, 0, 0, 0, 0, 0, 0]),
  );
  const ok = at(parseRawTransaction(raw, { idls: withArg("u64") }).parsed.instructions, 0);
  assert.equal(ok.decodeStatus, "decoded");
  assert.equal(ok.decoded?.args.x, "42");
  assert.equal(ok.decoded?.trailingBytes, 0);
});

test("fewer accounts than the IDL declares are named as missing", () => {
  const idls = counterIdls();
  const { signature } = incrementOf("counter_borsh");
  const raw = structuredClone(devnetRaw(signature));
  const message = raw.transaction.message;
  at(message.instructions, 0).accounts = [0];
  const decoded = at(parseRawTransaction(raw, { idls }).parsed.instructions, 0).decoded;
  assert.deepEqual(decoded?.missingAccounts, ["counter_account"]);
  assert.equal(decoded?.accounts.length, 1);
});

test("an event with an unknown discriminator or bad base64 is left undecoded with a reason", () => {
  const idls = counterIdls();
  const { signature } = incrementOf("counter_borsh");
  const withLog = (line: string) => {
    const raw = structuredClone(devnetRaw(signature));
    const logs = raw.meta?.logMessages as string[];
    const at1 = logs.findIndex((l) => l.startsWith("Program data: "));
    logs[at1] = line;
    return parseRawTransaction(raw, { idls }).parsed;
  };
  const unknown = withLog(`Program data: ${Buffer.from(new Uint8Array(24)).toString("base64")}`);
  assert.deepEqual(unknown.events, []);
  assert.match(at(unknown.undecodedLogData, 0).reason, /no event of counter_borsh/);

  const bad = withLog("Program data: !!!not base64!!!");
  assert.deepEqual(bad.events, []);
  assert.match(at(bad.undecodedLogData, 0).reason, /not valid base64/);
});

test("a log failure code that disagrees with the transaction's code is rejected", () => {
  const idls = counterIdls();
  const signature = Object.entries(index).find(
    ([, e]) => e.label === "increment with the wrong authority",
  )?.[0];
  assert.ok(signature);
  const raw = structuredClone(devnetRaw(signature));
  const logs = raw.meta?.logMessages as string[];
  const last = logs.length - 1;
  logs[last] = (logs[last] as string).replace("0x1770", "0x1771");
  assert.throws(
    () => parseRawTransaction(raw, { idls }),
    (error: unknown) => error instanceof Error && error.name === "ParseError",
  );
});
