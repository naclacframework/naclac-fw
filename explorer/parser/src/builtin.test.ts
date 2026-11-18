/**
 * Tests of the built-in program decoders: every instruction of the mainnet
 * fixture the reference decoded (System, Token, Associated Token Account,
 * Compute Budget) against the reference's values, plus byte-level tests of
 * every instruction layout and of malformed data. Run with
 * `node --test src/builtin.test.ts`.
 */

import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import { encodeBase58 } from "./base58.ts";
import { decodeAssociatedToken } from "./builtin/ata.ts";
import { decodeComputeBudget } from "./builtin/compute-budget.ts";
import { BUILTIN_PROGRAMS, decodeBuiltin } from "./builtin/index.ts";
import { decodeMemo } from "./builtin/memo.ts";
import type { BuiltinDecode } from "./builtin/reader.ts";
import { decodeSystem } from "./builtin/system.ts";
import { decodeToken } from "./builtin/token.ts";
import type { FlatInstruction } from "./flatten.ts";
import { parseNaclacIdl } from "./idl.ts";
import { parseRawTransaction } from "./parse.ts";
import type { AccountKey } from "./types.ts";
import {
  at,
  counterIdlDocuments,
  fixtures,
  mainnetRaw,
  mainnetSignature,
  readJson,
} from "./test-support.ts";

const bytes = (...parts: (number[] | Uint8Array)[]): Uint8Array =>
  Uint8Array.from(parts.flatMap((part) => Array.from(part)));
const u32le = (value: number): number[] => [
  value & 0xff,
  (value >>> 8) & 0xff,
  (value >>> 16) & 0xff,
  (value >>> 24) & 0xff,
];
const u64le = (value: bigint): number[] =>
  Array.from({ length: 8 }, (_, i) => Number((value >> BigInt(8 * i)) & 0xffn));
const keyBytes = (fill: number): number[] => new Array<number>(32).fill(fill);
const keyText = (fill: number): string => encodeBase58(Uint8Array.from(keyBytes(fill)));
const text = (value: string): number[] => Array.from(new TextEncoder().encode(value));
const string64 = (value: string): number[] => [...u64le(BigInt(text(value).length)), ...text(value)];

function succeeded(result: BuiltinDecode) {
  assert.ok(result.ok, JSON.stringify(result));
  return result;
}

function failed(result: BuiltinDecode) {
  assert.ok(!result.ok, JSON.stringify(result));
  return result;
}

test("mainnet: every instruction of a built-in program that the reference decoded matches it", () => {
  const reference = readJson(path.join(fixtures, `${mainnetSignature}.json`)).parsed.instructions;
  const programs = new Set(["system", "token", "compute_budget", "associated_token_account"]);
  const { parsed } = parseRawTransaction(mainnetRaw());
  const expected = reference.filter((r: any) => programs.has(r.programName) && r.decoded !== null);
  assert.ok(expected.length >= 17, `only ${expected.length} reference instructions`);

  for (const ref of expected) {
    const where = `${ref.instructionIndex}/${ref.innerInstructionIndex} ${ref.programName} ${ref.instructionName}`;
    const ours = parsed.instructions.find(
      (i) =>
        i.instructionIndex === ref.instructionIndex &&
        i.innerInstructionIndex === ref.innerInstructionIndex,
    );
    assert.ok(ours, where);
    assert.equal(ours.decodeStatus, "decoded", where);
    assert.equal(ours.programName, ref.programName, where);
    assert.equal(ours.instructionName, ref.instructionName, where);
    const decoded = ours.decoded;
    assert.ok(decoded, where);
    for (const [name, value] of Object.entries(ref.decoded.args)) {
      if (name === "extension_types") continue;
      assert.equal(String(decoded.args[name]), String(value), `${where} arg ${name}`);
    }
    assert.deepEqual(
      decoded.accounts.map((a) => a.pubkey).slice(0, ref.decoded.accounts.length),
      ref.decoded.accounts.map((a: any) => a.pubkey),
      where,
    );
  }
});

test("mainnet: the Token program's get_account_data_size carries two bytes it ignores", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const call = parsed.instructions.find((i) => i.instructionName === "get_account_data_size");
  assert.ok(call?.decoded);
  assert.deepEqual(call.decoded.args, {});
  assert.equal(call.decoded.trailingBytes, 2);
  const data = decodeToken("token-2022", bytes([21, 7, 0]));
  assert.deepEqual(succeeded(data).args, { extension_types: ["immutableOwner"] });
});

test("mainnet: accounts past a built-in program's list are named by position", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const limit = at(parsed.instructions, 0);
  assert.equal(limit.instructionName, "set_compute_unit_limit");
  assert.deepEqual(limit.decoded?.accounts.map((a) => a.name), ["remainingAccount1"]);
  assert.deepEqual(limit.decoded?.args, { units: 504294 });
  assert.equal(at(parsed.instructions, 1).decoded?.args.micro_lamports, "148723");
});

test("mainnet: instructions of programs that are not built in stay undecoded without an IDL", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const swap = parsed.instructions.find((i) => i.instructionIndex === 2 && i.innerInstructionIndex === null);
  assert.equal(swap?.decodeStatus, "no-idl");
  assert.equal(swap?.programName, null);
  assert.deepEqual(parsed.idls, []);
});

test("an IDL supplied for a built-in program's address is never used", () => {
  const document = structuredClone(counterIdlDocuments().counter_borsh);
  document.address = "11111111111111111111111111111111";
  const idl = parseNaclacIdl(document);
  const { parsed } = parseRawTransaction(mainnetRaw(), { idls: new Map([[idl.address, idl]]) });
  const transfer = parsed.instructions.find((i) => i.programId === idl.address);
  assert.equal(transfer?.programName, "system");
  assert.deepEqual(parsed.idls, []);
});

test("system: every instruction decodes with its name, fields and accounts", () => {
  const owner = keyBytes(7);
  const cases: [number, number[], string, Record<string, unknown>, string[]][] = [
    [0, [...u64le(5n), ...u64le(10n), ...owner], "create_account", { lamports: "5", space: "10", owner: keyText(7) }, ["funding_account", "new_account"]],
    [1, owner, "assign", { owner: keyText(7) }, ["assigned_account"]],
    [2, u64le(14936n), "transfer", { lamports: "14936" }, ["funding_account", "recipient_account"]],
    [4, [], "advance_nonce_account", {}, ["nonce_account", "recent_blockhashes_sysvar", "nonce_authority"]],
    [5, u64le(9n), "withdraw_nonce_account", { lamports: "9" }, ["nonce_account", "recipient_account", "recent_blockhashes_sysvar", "rent_sysvar", "nonce_authority"]],
    [6, owner, "initialize_nonce_account", { authority: keyText(7) }, ["nonce_account", "recent_blockhashes_sysvar", "rent_sysvar"]],
    [7, owner, "authorize_nonce_account", { new_authority: keyText(7) }, ["nonce_account", "nonce_authority"]],
    [8, u64le(33n), "allocate", { space: "33" }, ["new_account"]],
    [9, [...keyBytes(3), ...string64("ab"), ...u64le(8n), ...owner], "allocate_with_seed", { base: keyText(3), seed: "ab", space: "8", owner: keyText(7) }, ["allocated_account", "base_account"]],
    [10, [...keyBytes(3), ...string64("ab"), ...owner], "assign_with_seed", { base: keyText(3), seed: "ab", owner: keyText(7) }, ["assigned_account", "base_account"]],
    [11, [...u64le(4n), ...string64("xy"), ...owner], "transfer_with_seed", { lamports: "4", from_seed: "xy", from_owner: keyText(7) }, ["funding_account", "base_for_funding_account", "recipient_account"]],
    [12, [], "upgrade_nonce_account", {}, ["nonce_account"]],
  ];
  for (const [tag, body, name, args, accounts] of cases) {
    const result = succeeded(decodeSystem(bytes(u32le(tag), body), 9));
    assert.equal(result.name, name);
    assert.deepEqual(result.args, args, name);
    assert.deepEqual(result.accounts, accounts, name);
    assert.equal(result.trailingBytes, 0, name);
  }
});

test("system: accounts that depend on the instruction's contents", () => {
  const withSeed = bytes(u32le(3), keyBytes(3), string64("s"), u64le(1n), u64le(2n), keyBytes(7));
  assert.deepEqual(succeeded(decodeSystem(withSeed, 3)).accounts, ["funding_account", "created_account", "base_account"]);
  assert.deepEqual(succeeded(decodeSystem(withSeed, 2)).accounts, ["funding_account", "created_account"]);
  const prefund = (lamports: bigint) => bytes(u32le(13), u64le(lamports), u64le(2n), keyBytes(7));
  assert.deepEqual(succeeded(decodeSystem(prefund(0n), 1)).accounts, ["new_account"]);
  assert.deepEqual(succeeded(decodeSystem(prefund(5n), 2)).accounts, ["new_account", "funding_account"]);
});

test("system: malformed data is reported, never guessed", () => {
  assert.equal(failed(decodeSystem(bytes([2, 0]), 2)).status, "unknown-instruction");
  assert.equal(failed(decodeSystem(bytes(u32le(14)), 2)).status, "unknown-instruction");
  assert.equal(failed(decodeSystem(bytes(u32le(2), [1, 2, 3]), 2)).status, "decode-error");
  const longString = bytes(u32le(9), keyBytes(3), u64le(99n), text("ab"), u64le(1n), keyBytes(7));
  assert.match(failed(decodeSystem(longString, 2)).reason, /declares 99 bytes/);
  const badUtf8 = bytes(u32le(10), keyBytes(3), u64le(1n), [0xff], keyBytes(7));
  assert.match(failed(decodeSystem(badUtf8, 2)).reason, /not valid UTF-8/);
  assert.equal(succeeded(decodeSystem(bytes(u32le(2), u64le(1n), [9, 9]), 2)).trailingBytes, 2);
});

test("compute budget: every instruction", () => {
  assert.deepEqual(succeeded(decodeComputeBudget(bytes([1], u32le(32768)))).args, { bytes: 32768 });
  assert.deepEqual(succeeded(decodeComputeBudget(bytes([2], u32le(257)))).args, { units: 257 });
  assert.deepEqual(succeeded(decodeComputeBudget(bytes([3], u64le(2n ** 64n - 1n)))).args, {
    micro_lamports: "18446744073709551615",
  });
  assert.deepEqual(succeeded(decodeComputeBudget(bytes([4], u32le(64)))).args, { bytes: 64 });
  assert.equal(succeeded(decodeComputeBudget(bytes([0]))).name, "unused");
  assert.equal(failed(decodeComputeBudget(bytes([5]))).status, "unknown-instruction");
  assert.equal(failed(decodeComputeBudget(bytes())).status, "unknown-instruction");
  assert.equal(failed(decodeComputeBudget(bytes([2, 1, 0]))).status, "decode-error");
  assert.equal(succeeded(decodeComputeBudget(bytes([2], u32le(1), [0]))).trailingBytes, 1);
});

test("associated token account: create, create idempotent, recover nested, and empty data", () => {
  for (const data of [bytes(), bytes([0])]) {
    const result = succeeded(decodeAssociatedToken(data));
    assert.equal(result.name, "create");
    assert.equal(result.accounts.length, 6);
  }
  assert.equal(succeeded(decodeAssociatedToken(bytes([1]))).name, "create_idempotent");
  const recover = succeeded(decodeAssociatedToken(bytes([2])));
  assert.equal(recover.name, "recover_nested");
  assert.equal(recover.accounts.length, 7);
  assert.equal(failed(decodeAssociatedToken(bytes([3]))).status, "unknown-instruction");
  assert.equal(succeeded(decodeAssociatedToken(bytes([0, 9]))).trailingBytes, 1);
});

test("memo: the whole data is the text, and invalid UTF-8 is refused", () => {
  assert.deepEqual(succeeded(decodeMemo(new TextEncoder().encode("héllo 🙂"))).args, { memo: "héllo 🙂" });
  assert.deepEqual(succeeded(decodeMemo(bytes())).args, { memo: "" });
  assert.match(failed(decodeMemo(bytes([0xff, 0xfe]))).reason, /not valid UTF-8/);
});

test("memo: versions 1 and 3 are built in, version 4 is not, and signers are named", () => {
  assert.ok(BUILTIN_PROGRAMS.has("Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"));
  assert.ok(BUILTIN_PROGRAMS.has("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"));
  assert.ok(!BUILTIN_PROGRAMS.has("Memo4c2pN8afCj432Lb7RMVKi9PbQnnW7ewFFaV3oAH"));

  const keys: AccountKey[] = ["MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr", keyText(1), keyText(2)].map(
    (address, index) => ({ index, address, signer: index > 0, writable: false, source: "static" }),
  );
  const instruction: FlatInstruction = {
    instructionIndex: 0,
    innerInstructionIndex: null,
    stackHeight: 1,
    programId: keys[0]?.address as string,
    rawAccounts: [keyText(1), keyText(2)],
    rawData: encodeBase58(new TextEncoder().encode("hi")),
    accountIndexes: [1, 2],
  };
  const result = decodeBuiltin(instruction, keys);
  assert.equal(result?.decodeStatus, "decoded");
  assert.equal(result?.programName, "memo");
  assert.deepEqual(result?.decoded?.accounts.map((a) => a.name), ["signer1", "signer2"]);
  assert.ok(result?.decoded?.accounts.every((a) => a.isSigner));
});

test("a built-in instruction missing accounts names them", () => {
  const keys: AccountKey[] = ["11111111111111111111111111111111", keyText(1)].map((address, index) => ({
    index,
    address,
    signer: index === 1,
    writable: index === 1,
    source: "static",
  }));
  const instruction: FlatInstruction = {
    instructionIndex: 0,
    innerInstructionIndex: null,
    stackHeight: 1,
    programId: "11111111111111111111111111111111",
    rawAccounts: [keyText(1)],
    rawData: encodeBase58(bytes(u32le(0), u64le(1n), u64le(2n), keyBytes(7))),
    accountIndexes: [1],
  };
  const result = decodeBuiltin(instruction, keys);
  assert.equal(result?.decoded?.accounts.length, 1);
  assert.deepEqual(result?.decoded?.missingAccounts, ["new_account"]);
});

test("token: instructions with amounts, decimals, owners and options", () => {
  const cases: [number[], string, Record<string, unknown>, string[]][] = [
    [[3, ...u64le(199222826018n)], "transfer", { amount: "199222826018" }, ["source", "destination", "authority"]],
    [[4, ...u64le(1n)], "approve", { amount: "1" }, ["source", "delegate", "owner"]],
    [[7, ...u64le(2n)], "mint_to", { amount: "2" }, ["mint", "destination", "mint_authority"]],
    [[8, ...u64le(3n)], "burn", { amount: "3" }, ["source", "mint", "authority"]],
    [[12, ...u64le(4n), 6], "transfer_checked", { amount: "4", decimals: 6 }, ["source", "mint", "destination", "authority"]],
    [[13, ...u64le(4n), 6], "approve_checked", { amount: "4", decimals: 6 }, ["source", "mint", "delegate", "owner"]],
    [[14, ...u64le(4n), 6], "mint_to_checked", { amount: "4", decimals: 6 }, ["mint", "destination", "mint_authority"]],
    [[15, ...u64le(4n), 6], "burn_checked", { amount: "4", decimals: 6 }, ["source", "mint", "authority"]],
    [[16, ...keyBytes(5)], "initialize_account_2", { owner: keyText(5) }, ["account", "mint", "rent_sysvar"]],
    [[18, ...keyBytes(5)], "initialize_account_3", { owner: keyText(5) }, ["account", "mint"]],
    [[1], "initialize_account", {}, ["account", "mint", "owner", "rent_sysvar"]],
    [[2, 3], "initialize_multisig", { m: 3 }, ["multisig", "rent_sysvar"]],
    [[19, 2], "initialize_multisig_2", { m: 2 }, ["multisig"]],
    [[5], "revoke", {}, ["source", "owner"]],
    [[9], "close_account", {}, ["account", "destination", "owner"]],
    [[10], "freeze_account", {}, ["account", "mint", "freeze_authority"]],
    [[11], "thaw_account", {}, ["account", "mint", "freeze_authority"]],
    [[17], "sync_native", {}, ["account"]],
    [[22], "initialize_immutable_owner", {}, ["account"]],
    [[23, ...u64le(9n)], "amount_to_ui_amount", { amount: "9" }, ["mint"]],
    [[38], "withdraw_excess_lamports", {}, ["source", "destination", "authority"]],
  ];
  for (const flavor of ["token", "token-2022"] as const) {
    for (const [data, name, args, accounts] of cases) {
      const result = succeeded(decodeToken(flavor, bytes(data)));
      assert.equal(result.name, name, `${flavor} ${name}`);
      assert.deepEqual(result.args, args, `${flavor} ${name}`);
      assert.deepEqual(result.accounts, accounts, `${flavor} ${name}`);
    }
  }
});

test("token: mint initialization and optional fields", () => {
  const none = succeeded(decodeToken("token", bytes([20, 9], keyBytes(1), [0])));
  assert.deepEqual(none.args, { decimals: 9, mint_authority: keyText(1), freeze_authority: null });
  const some = succeeded(decodeToken("token", bytes([0, 6], keyBytes(1), [1], keyBytes(2))));
  assert.deepEqual(some.args, { decimals: 6, mint_authority: keyText(1), freeze_authority: keyText(2) });
  assert.match(failed(decodeToken("token", bytes([20, 9], keyBytes(1), [2]))).reason, /not 0 or 1/);
  assert.equal(failed(decodeToken("token", bytes([20, 9], keyBytes(1), [1], keyBytes(2).slice(0, 10)))).status, "decode-error");
  assert.deepEqual(succeeded(decodeToken("token", bytes([45, 0]))).args, { amount: null });
  assert.deepEqual(succeeded(decodeToken("token", bytes([45, 1], u64le(7n)))).args, { amount: "7" });
});

test("token: set_authority names the authority type for each program", () => {
  const set = (type: number) => bytes([6, type, 0]);
  assert.deepEqual(succeeded(decodeToken("token", set(2))).args, { authority_type: "account_owner", new_authority: null });
  assert.equal(failed(decodeToken("token", set(4))).status, "decode-error");
  assert.deepEqual(succeeded(decodeToken("token-2022", set(4))).args.authority_type, "transfer_fee_config");
  assert.deepEqual(succeeded(decodeToken("token-2022", set(17))).args.authority_type, "permissioned_burn");
  assert.equal(failed(decodeToken("token-2022", set(18))).status, "decode-error");
});

test("token: extension type lists and UI amounts", () => {
  assert.deepEqual(succeeded(decodeToken("token", bytes([21, 7, 0]))).args, {});
  assert.equal(succeeded(decodeToken("token", bytes([21, 7, 0]))).trailingBytes, 2);
  assert.deepEqual(succeeded(decodeToken("token-2022", bytes([21]))).args, { extension_types: [] });
  assert.deepEqual(succeeded(decodeToken("token-2022", bytes([29, 7, 0, 8, 0]))).args, {
    extension_types: ["immutableOwner", "memoTransfer"],
  });
  assert.equal(failed(decodeToken("token-2022", bytes([21, 7]))).status, "decode-error");
  assert.match(failed(decodeToken("token-2022", bytes([21, 99, 0]))).reason, /not an extension type/);
  assert.deepEqual(succeeded(decodeToken("token", bytes([24], text("1.5")))).args, { ui_amount: "1.5" });
  assert.match(failed(decodeToken("token", bytes([24, 0xff]))).reason, /not valid UTF-8/);
});

test("token-2022: the instructions it adds, and ones that are only named", () => {
  assert.deepEqual(succeeded(decodeToken("token-2022", bytes([25, 1], keyBytes(4)))).args, {
    close_authority: keyText(4),
  });
  assert.deepEqual(succeeded(decodeToken("token-2022", bytes([35], keyBytes(4)))).args, {
    delegate: keyText(4),
  });
  assert.deepEqual(succeeded(decodeToken("token-2022", bytes([31]))).accounts, [
    "funding_account",
    "native_mint",
    "system_program",
  ]);
  const extension = failed(decodeToken("token-2022", bytes([26, 0])));
  assert.equal(extension.status, "unsupported-type");
  assert.equal(extension.name, "transfer_fee_extension");
  for (const flavor of ["token", "token-2022"] as const) {
    const batch = failed(decodeToken(flavor, bytes([255, 1, 1, 3])));
    assert.equal(batch.status, "unsupported-type");
    assert.equal(batch.name, "batch");
  }
});

test("token: tags a program does not have, and empty data, are unknown", () => {
  for (const tag of [25, 26, 31, 35, 37, 46, 100]) {
    assert.equal(failed(decodeToken("token", bytes([tag]))).status, "unknown-instruction", `token ${tag}`);
  }
  assert.equal(failed(decodeToken("token-2022", bytes([100]))).status, "unknown-instruction");
  assert.equal(failed(decodeToken("token-2022", bytes([47]))).status, "unknown-instruction");
  assert.equal(failed(decodeToken("token", bytes())).status, "unknown-instruction");
  assert.equal(failed(decodeToken("token", bytes([3, 1, 2]))).status, "decode-error");
  assert.equal(succeeded(decodeToken("token", bytes([3], u64le(1n), [9]))).trailingBytes, 1);
});
