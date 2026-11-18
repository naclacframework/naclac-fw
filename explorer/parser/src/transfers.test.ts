/**
 * Tests of token balance changes and of transfer extraction: the mainnet
 * fixture against the reference's transfers and against the node's own balance
 * changes, the lamport model against the real fee payer balance, every devnet
 * fixture, and synthetic transactions for each rule. Run with
 * `node --test src/transfers.test.ts`.
 */

import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import { ParseError } from "./errors.ts";
import { parseRawTransaction } from "./parse.ts";
import { tokenBalanceChanges } from "./token-balances.ts";
import { extractTransfers } from "./transfers.ts";
import type { AccountKey, ArgValue, ParsedInstruction, RpcMeta, TokenBalanceChange } from "./types.ts";
import {
  at,
  devnetRaw,
  fixtures,
  index,
  mainnetRaw,
  mainnetSignature,
  readJson,
} from "./test-support.ts";

const NATIVE = "So11111111111111111111111111111111111111112";
const TOKEN = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

function assertParseError(action: () => unknown, field: string): void {
  assert.throws(
    action,
    (error: unknown) => error instanceof ParseError && error.field === field,
    `expected a ParseError for ${field}`,
  );
}

test("mainnet: the token transfers equal the reference's, in mint, token accounts and amount", () => {
  const reference = readJson(path.join(fixtures, `${mainnetSignature}.json`)).parsed.tokenTransfers;
  const { parsed } = parseRawTransaction(mainnetRaw());
  assert.equal(parsed.transfersCommitted, true);
  assert.equal(parsed.transfersComplete, true);
  assert.deepEqual(parsed.unexplainedLamportChanges, []);
  assert.equal(parsed.tokenTransfers.length, reference.length);
  assert.equal(reference.length, 8);
  for (const ref of reference) {
    const ours = parsed.tokenTransfers.find(
      (t) =>
        t.fromTokenAccount === ref.fromTokenAccount &&
        t.toTokenAccount === ref.toTokenAccount &&
        t.amount === String(ref.rawTokenAmount),
    );
    assert.ok(ours, `no transfer ${ref.fromTokenAccount} -> ${ref.toTokenAccount} of ${ref.rawTokenAmount}`);
    assert.equal(ours.mint, ref.mint);
    assert.equal(ours.decimals, ref.decimals);
    assert.equal(ours.kind, "transfer");
  }
});

test("mainnet: the native transfers equal the reference's, including the closed account's balance", () => {
  const reference = readJson(path.join(fixtures, `${mainnetSignature}.json`)).parsed.nativeTransfers;
  const { parsed } = parseRawTransaction(mainnetRaw());
  assert.equal(parsed.nativeTransfers.length, reference.length);
  assert.equal(reference.length, 3);
  for (const ref of reference) {
    const ours = parsed.nativeTransfers.find(
      (t) => t.from === ref.fromUserAccount && t.to === ref.toUserAccount && t.lamports === String(ref.amount),
    );
    assert.ok(ours, `no transfer ${ref.fromUserAccount} -> ${ref.toUserAccount} of ${ref.amount}`);
  }
  assert.deepEqual(
    parsed.nativeTransfers.map((t) => t.kind),
    ["create_account", "close_account", "transfer"],
  );
});

test("mainnet: the lamport model reproduces the fee payer's real final balance", () => {
  const raw = mainnetRaw();
  const { parsed } = parseRawTransaction(raw);
  const payer = parsed.feePayer;
  let delta = 0n;
  for (const transfer of parsed.nativeTransfers) {
    if (transfer.to === payer) delta += BigInt(transfer.lamports);
    if (transfer.from === payer) delta -= BigInt(transfer.lamports);
  }
  const pre = BigInt(raw.meta?.preBalances[0] ?? 0);
  const post = BigInt(raw.meta?.postBalances[0] ?? 0);
  assert.equal(pre - BigInt(parsed.fee) + delta, post);
});

test("mainnet: token transfers net out to each token account's balance change", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const changes = parsed.tokenBalanceChanges;
  assert.ok(changes);
  assert.equal(changes.length, 15);
  const net = new Map<string, bigint>();
  for (const t of parsed.tokenTransfers) {
    net.set(t.toTokenAccount, (net.get(t.toTokenAccount) ?? 0n) + BigInt(t.amount));
    net.set(t.fromTokenAccount, (net.get(t.fromTokenAccount) ?? 0n) - BigInt(t.amount));
  }
  for (const change of changes) {
    assert.equal(net.get(change.address) ?? 0n, BigInt(change.change), change.address);
  }
  const unlisted = [...net.keys()].filter((address) => !changes.some((c) => c.address === address));
  assert.deepEqual(unlisted, ["HpSRJZok15uyhuM2wHgNjEEZm8fFyMF2RxLPctWXVLm1"]);
});

test("mainnet: each mint's balances are conserved, except wrapped SOL leaving as lamports", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const sums = new Map<string, bigint>();
  for (const change of parsed.tokenBalanceChanges ?? []) {
    sums.set(change.mint, (sums.get(change.mint) ?? 0n) + BigInt(change.change));
  }
  for (const [mint, sum] of sums) {
    assert.equal(sum, mint === NATIVE ? -488052565n : 0n, mint);
  }
  const closed = parsed.nativeTransfers.find((t) => t.kind === "close_account");
  assert.equal(closed?.lamports, "489541005");
  const wrapped = parsed.tokenTransfers.filter((t) => t.toTokenAccount === closed?.from);
  assert.ok(wrapped.length > 0);
});

test("mainnet: owners come from the balances and from the account setup in the transaction", () => {
  const { parsed } = parseRawTransaction(mainnetRaw());
  const user = "EBgTpjGvic1PP4WFf4QYX6qXKQxVkhkQhtyeaVvpfLu4";
  const setup = parsed.tokenTransfers.find(
    (t) => t.toTokenAccount === "6UGeCpf56g8N3douofsQoBd71xjoNMATJnC7HkuXXWbL",
  );
  assert.equal(setup?.fromOwner, user);
  assert.equal(setup?.toOwner, "BhVHbM8cn9ojSi2VS1HTCQzDM1hspkviLFDvF7quv3kR");
  const toNew = parsed.tokenTransfers.find(
    (t) => t.toTokenAccount === "HpSRJZok15uyhuM2wHgNjEEZm8fFyMF2RxLPctWXVLm1",
  );
  assert.equal(toNew?.toOwner, user);
  assert.equal(toNew?.mint, NATIVE);
});

test("every devnet fixture: successes are committed and complete, failures extract nothing", () => {
  for (const [signature, entry] of Object.entries(index)) {
    const { parsed } = parseRawTransaction(devnetRaw(signature));
    const where = `${entry.program} / ${entry.label}`;
    assert.equal(parsed.transfersCommitted, entry.outcome === "succeeded", where);
    assert.equal(parsed.transfersComplete, true, where);
    assert.deepEqual(parsed.unexplainedLamportChanges, [], where);
    assert.deepEqual(parsed.nativeTransfers, [], where);
    assert.deepEqual(parsed.tokenTransfers, [], where);
    assert.deepEqual(parsed.tokenBalanceChanges, [], where);
  }
});

function keysFor(addresses: string[]): AccountKey[] {
  return addresses.map((address, index) => ({
    index,
    address,
    signer: index === 0,
    writable: true,
    source: "static",
  }));
}

function instruction(options: {
  program: string;
  programId?: string;
  name: string | null;
  args?: Record<string, ArgValue>;
  accounts?: [string, string][];
  index?: number;
  inner?: number | null;
}): ParsedInstruction {
  return {
    instructionIndex: options.index ?? 0,
    innerInstructionIndex: options.inner ?? null,
    stackHeight: 1,
    programId: options.programId ?? options.program,
    rawAccounts: [],
    rawData: "",
    programName: options.program,
    instructionName: options.name,
    decoded:
      options.accounts === undefined && options.args === undefined
        ? null
        : {
            args: options.args ?? {},
            accounts: (options.accounts ?? []).map(([name, pubkey]) => ({
              name,
              pubkey,
              isSigner: false,
              isWritable: true,
              provided: true,
            })),
            missingAccounts: [],
            trailingBytes: 0,
          },
    decodeStatus: options.accounts === undefined && options.args === undefined ? "unsupported-type" : "decoded",
    decodeReason: null,
  };
}

const system = (name: string, args: Record<string, ArgValue>, accounts: [string, string][], inner: number | null = null) =>
  instruction({ program: "system", programId: "11111111111111111111111111111111", name, args, accounts, inner });
const token = (name: string, args: Record<string, ArgValue>, accounts: [string, string][], inner: number | null = null) =>
  instruction({ program: "token", programId: TOKEN, name, args, accounts, inner });

test("a failed transaction extracts nothing and says so", () => {
  const result = extractTransfers({
    succeeded: false,
    instructions: [system("transfer", { lamports: "5" }, [["funding_account", "A"], ["recipient_account", "B"]])],
    keys: keysFor(["A", "B"]),
    preLamports: [10n, 0n],
    postLamports: [10n, 0n],
    fee: 0n,
    tokenChanges: null,
  });
  assert.equal(result.committed, false);
  assert.deepEqual(result.nativeTransfers, []);
});

test("system transfers are recorded with exact amounts, and zero amounts are not", () => {
  const result = extractTransfers({
    succeeded: true,
    instructions: [
      system("transfer", { lamports: "5" }, [["funding_account", "A"], ["recipient_account", "B"]]),
      system("transfer", { lamports: "0" }, [["funding_account", "A"], ["recipient_account", "B"]]),
      system("create_account", { lamports: "7", space: "0", owner: "X" }, [["funding_account", "A"], ["new_account", "C"]]),
      system("withdraw_nonce_account", { lamports: "3" }, [["nonce_account", "N"], ["recipient_account", "B"]]),
      system("allocate", { space: "9" }, [["new_account", "C"]]),
    ],
    keys: keysFor(["A", "B", "C", "N"]),
    preLamports: [100n, 0n, 0n, 50n],
    postLamports: [88n, 8n, 7n, 47n],
    fee: 0n,
    tokenChanges: null,
  });
  assert.deepEqual(
    result.nativeTransfers.map((t) => [t.kind, t.from, t.to, t.lamports]),
    [
      ["transfer", "A", "B", "5"],
      ["create_account", "A", "C", "7"],
      ["withdraw_nonce_account", "N", "B", "3"],
    ],
  );
  assert.equal(result.complete, true);
});

test("a closed wrapped-SOL account's lamports are tracked through create, token transfer and close", () => {
  const result = extractTransfers({
    succeeded: true,
    instructions: [
      system("create_account", { lamports: "50", space: "165", owner: TOKEN }, [["funding_account", "P"], ["new_account", "T1"]], null),
      token("initialize_account_3", { owner: "P" }, [["account", "T1"], ["mint", NATIVE]]),
      token("transfer_checked", { amount: "30", decimals: 9 }, [["source", "T2"], ["mint", NATIVE], ["destination", "T1"], ["authority", "P"]]),
      token("close_account", {}, [["account", "T1"], ["destination", "P"], ["owner", "P"]]),
    ],
    keys: keysFor(["P", "T1", "T2"]),
    preLamports: [1000n, 0n, 100n],
    postLamports: [1030n, 0n, 70n],
    fee: 0n,
    tokenChanges: null,
  });
  assert.deepEqual(
    result.nativeTransfers.map((t) => [t.kind, t.from, t.to, t.lamports]),
    [
      ["create_account", "P", "T1", "50"],
      ["close_account", "T1", "P", "80"],
    ],
  );
  const transfer = at(result.tokenTransfers, 0);
  assert.equal(transfer.mint, NATIVE);
  assert.equal(transfer.decimals, 9);
  assert.equal(transfer.kind, "transfer_checked");
  assert.equal(transfer.toOwner, "P");
  assert.equal(result.complete, true);
});

test("a token transfer between ordinary accounts moves no lamports", () => {
  const result = extractTransfers({
    succeeded: true,
    instructions: [
      token("transfer", { amount: "5" }, [["source", "S"], ["destination", "D"], ["authority", "P"]]),
      token("close_account", {}, [["account", "S"], ["destination", "P"], ["owner", "P"]]),
    ],
    keys: keysFor(["P", "S", "D"]),
    preLamports: [0n, 2039280n, 2039280n],
    postLamports: [2039280n, 0n, 2039280n],
    fee: 0n,
    tokenChanges: [
      { index: 1, address: "S", mint: "M", owner: "P", programId: TOKEN, decimals: 6, pre: "5", post: null, change: "-5" },
    ],
  });
  assert.equal(at(result.tokenTransfers, 0).mint, "M");
  assert.equal(at(result.tokenTransfers, 0).decimals, 6);
  assert.equal(at(result.tokenTransfers, 0).fromOwner, "P");
  assert.equal(at(result.nativeTransfers, 0).lamports, "2039280");
  assert.equal(result.complete, true);
});

test("a transfer whose mint is unknown makes the extraction incomplete", () => {
  const result = extractTransfers({
    succeeded: true,
    instructions: [token("transfer", { amount: "5" }, [["source", "S"], ["destination", "D"], ["authority", "P"]])],
    keys: keysFor(["P", "S", "D"]),
    preLamports: [0n, 0n, 0n],
    postLamports: [0n, 0n, 0n],
    fee: 0n,
    tokenChanges: null,
  });
  assert.equal(at(result.tokenTransfers, 0).mint, null);
  assert.equal(result.complete, false);
});

test("instructions whose movement is not modelled make the extraction incomplete", () => {
  for (const name of ["unwrap_lamports", "withdraw_excess_lamports"]) {
    const result = extractTransfers({
      succeeded: true,
      instructions: [token(name, { amount: null }, [["source", "S"], ["destination", "D"], ["authority", "P"]])],
      keys: keysFor(["P", "S", "D"]),
      preLamports: [0n, 10n, 0n],
      postLamports: [0n, 10n, 0n],
      fee: 0n,
      tokenChanges: null,
    });
    assert.equal(result.complete, false, name);
    assert.deepEqual(result.nativeTransfers, [], name);
  }
  const batch = extractTransfers({
    succeeded: true,
    instructions: [instruction({ program: "token", programId: TOKEN, name: "batch" })],
    keys: keysFor(["P"]),
    preLamports: [0n],
    postLamports: [0n],
    fee: 0n,
    tokenChanges: null,
  });
  assert.equal(batch.complete, false);
});

test("an associated token account created in the transaction gives its owner and mint", () => {
  const result = extractTransfers({
    succeeded: true,
    instructions: [
      instruction({
        program: "associated_token_account",
        programId: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
        name: "create_idempotent",
        args: {},
        accounts: [["funding_account", "P"], ["associated_token_account", "ATA"], ["wallet_address", "W"], ["token_mint", "M"]],
      }),
      token("transfer", { amount: "1" }, [["source", "S"], ["destination", "ATA"], ["authority", "P"]]),
    ],
    keys: keysFor(["P", "ATA", "S", "W", "M"]),
    preLamports: [0n, 0n, 0n, 0n, 0n],
    postLamports: [0n, 0n, 0n, 0n, 0n],
    fee: 0n,
    tokenChanges: [
      { index: 2, address: "S", mint: "M", owner: "P", programId: TOKEN, decimals: 0, pre: "1", post: "0", change: "-1" },
    ],
  });
  const transfer = at(result.tokenTransfers, 0);
  assert.equal(transfer.toOwner, "W");
  assert.equal(transfer.mint, "M");
});

function metaWith(overrides: Partial<RpcMeta>): RpcMeta {
  return { err: null, fee: 0, preBalances: [], postBalances: [], ...overrides };
}

const entry = (accountIndex: number, amount: string, mint = "M") => ({
  accountIndex,
  mint,
  owner: "O",
  programId: TOKEN,
  uiTokenAmount: { amount, decimals: 6 },
});

test("token balance changes: merged by account, created and closed accounts have a null side", () => {
  const keys = keysFor(["a", "b", "c"]);
  const changes = tokenBalanceChanges(
    metaWith({
      preTokenBalances: [entry(1, "10"), entry(2, "5")],
      postTokenBalances: [entry(1, "4"), entry(0, "6")],
    }),
    keys,
  ) as TokenBalanceChange[];
  assert.deepEqual(
    changes.map((c) => [c.index, c.pre, c.post, c.change]),
    [
      [0, null, "6", "6"],
      [1, "10", "4", "-6"],
      [2, "5", null, "-5"],
    ],
  );
  assert.equal(tokenBalanceChanges(metaWith({}), keys), null);
  assert.deepEqual(tokenBalanceChanges(metaWith({ preTokenBalances: [], postTokenBalances: [] }), keys), []);
});

test("token balance changes: malformed entries are rejected with the offending field", () => {
  const keys = keysFor(["a", "b"]);
  const bad = (pre: unknown[], post: unknown[] = []) =>
    tokenBalanceChanges(metaWith({ preTokenBalances: pre as never, postTokenBalances: post as never }), keys);
  assertParseError(() => tokenBalanceChanges(metaWith({ preTokenBalances: [] }), keys), "meta.preTokenBalances");
  assertParseError(() => bad([entry(5, "1")]), "meta.preTokenBalances[0]");
  assertParseError(() => bad([entry(0, "-1")]), "meta.preTokenBalances[0]");
  assertParseError(() => bad([entry(0, "1.5")]), "meta.preTokenBalances[0]");
  assertParseError(() => bad([entry(0, "1"), entry(0, "2")]), "meta.preTokenBalances");
  assertParseError(() => bad([{ ...entry(0, "1"), mint: "" }]), "meta.preTokenBalances[0]");
  assertParseError(() => bad([{ ...entry(0, "1"), uiTokenAmount: { amount: "1", decimals: 300 } }]), "meta.preTokenBalances[0]");
  assertParseError(() => bad([entry(0, "1", "A")], [entry(0, "1", "B")]), "meta.postTokenBalances");
});

test("lamports a program moved directly are reported as unexplained, and the fee is not", () => {
  const closedByProgram = extractTransfers({
    succeeded: true,
    instructions: [],
    keys: keysFor(["P", "STATE"]),
    preLamports: [1000n, 400n],
    postLamports: [1395n, 0n],
    fee: 5n,
    tokenChanges: null,
  });
  assert.deepEqual(
    closedByProgram.unexplained.map((u) => [u.address, u.expected, u.actual, u.difference]),
    [
      ["P", "995", "1395", "400"],
      ["STATE", "400", "0", "-400"],
    ],
  );
  assert.equal(closedByProgram.complete, false);

  const feeOnly = extractTransfers({
    succeeded: true,
    instructions: [],
    keys: keysFor(["P", "X"]),
    preLamports: [1000n, 7n],
    postLamports: [995n, 7n],
    fee: 5n,
    tokenChanges: null,
  });
  assert.deepEqual(feeOnly.unexplained, []);
  assert.equal(feeOnly.complete, true);
});
