/**
 * Native and token transfers, read from the decoded System, Token and
 * Token-2022 instructions (top level and inner) of a transaction that
 * succeeded. Lamport amounts that no instruction states, such as a closed
 * token account's balance, come from tracking lamports through the
 * instructions, starting from each account's pre-transaction balance. Token
 * accounts' mints and owners come from the node's token balances and from the
 * account setup instructions in the transaction itself.
 */

import type {
  AccountKey,
  NativeTransfer,
  ParsedInstruction,
  TokenBalanceChange,
  TokenTransfer,
  UnexplainedLamportChange,
} from "./types.ts";

const NATIVE_MINT = "So11111111111111111111111111111111111111112";

interface TokenAccountInfo {
  mint: string | null;
  owner: string | null;
  decimals: number | null;
}

export interface Transfers {
  nativeTransfers: NativeTransfer[];
  tokenTransfers: TokenTransfer[];
  committed: boolean;
  complete: boolean;
  unexplained: UnexplainedLamportChange[];
}

export interface TransferInput {
  succeeded: boolean;
  instructions: ParsedInstruction[];
  keys: AccountKey[];
  preLamports: bigint[];
  postLamports: bigint[];
  fee: bigint;
  tokenChanges: TokenBalanceChange[] | null;
}

const LAMPORT_MOVES = new Map<string, [string, string]>([
  ["create_account", ["funding_account", "new_account"]],
  ["create_account_with_seed", ["funding_account", "created_account"]],
  ["create_account_allow_prefund", ["funding_account", "new_account"]],
  ["transfer", ["funding_account", "recipient_account"]],
  ["transfer_with_seed", ["funding_account", "recipient_account"]],
  ["withdraw_nonce_account", ["nonce_account", "recipient_account"]],
]);

const UNMODELED_TOKEN_INSTRUCTIONS = new Set(["batch", "unwrap_lamports", "withdraw_excess_lamports"]);

function accountNamed(instruction: ParsedInstruction, name: string): string | undefined {
  return instruction.decoded?.accounts.find((account) => account.name === name)?.pubkey;
}

function stringArg(instruction: ParsedInstruction, name: string): string | undefined {
  const value = instruction.decoded?.args[name];
  return typeof value === "string" ? value : undefined;
}

function isToken(instruction: ParsedInstruction): boolean {
  return instruction.programName === "token" || instruction.programName === "token_2022";
}

function tokenAccountRegistry(
  instructions: ParsedInstruction[],
  tokenChanges: TokenBalanceChange[] | null,
): Map<string, TokenAccountInfo> {
  const registry = new Map<string, TokenAccountInfo>();
  const learn = (address: string | undefined, found: Partial<TokenAccountInfo>): void => {
    if (address === undefined) return;
    const known = registry.get(address) ?? { mint: null, owner: null, decimals: null };
    registry.set(address, {
      mint: known.mint ?? found.mint ?? null,
      owner: known.owner ?? found.owner ?? null,
      decimals: known.decimals ?? found.decimals ?? null,
    });
  };

  for (const change of tokenChanges ?? []) {
    learn(change.address, { mint: change.mint, owner: change.owner, decimals: change.decimals });
  }
  for (const instruction of instructions) {
    const name = instruction.instructionName;
    if (isToken(instruction) && name?.startsWith("initialize_account")) {
      learn(accountNamed(instruction, "account"), {
        mint: accountNamed(instruction, "mint") ?? null,
        owner: accountNamed(instruction, "owner") ?? stringArg(instruction, "owner") ?? null,
      });
    } else if (
      instruction.programName === "associated_token_account" &&
      (name === "create" || name === "create_idempotent")
    ) {
      learn(accountNamed(instruction, "associated_token_account"), {
        mint: accountNamed(instruction, "token_mint") ?? null,
        owner: accountNamed(instruction, "wallet_address") ?? null,
      });
    }
  }
  return registry;
}

/**
 * Extracts the native and token transfers of a transaction. Nothing is
 * extracted from a failed transaction (`committed` is false). `complete` is
 * false when the transaction holds an instruction whose movement is not
 * modelled (a token batch, `unwrap_lamports`, `withdraw_excess_lamports`), a
 * token transfer whose mint is unknown, so wrapped-SOL lamport movement cannot
 * be ruled out, or an account whose final lamports the tracked movement and the
 * fee do not explain (listed in `unexplained`).
 */
export function extractTransfers(input: TransferInput): Transfers {
  const result: Transfers = {
    nativeTransfers: [],
    tokenTransfers: [],
    committed: input.succeeded,
    complete: true,
    unexplained: [],
  };
  if (!input.succeeded) return result;

  const registry = tokenAccountRegistry(input.instructions, input.tokenChanges);
  const lamports = new Map<string, bigint>();
  for (const key of input.keys) lamports.set(key.address, input.preLamports[key.index] ?? 0n);
  const move = (from: string, to: string, amount: bigint): void => {
    lamports.set(from, (lamports.get(from) ?? 0n) - amount);
    lamports.set(to, (lamports.get(to) ?? 0n) + amount);
  };

  for (const instruction of input.instructions) {
    const where = {
      instructionIndex: instruction.instructionIndex,
      innerInstructionIndex: instruction.innerInstructionIndex,
      programId: instruction.programId,
    };
    const name = instruction.instructionName;

    if (name !== null && UNMODELED_TOKEN_INSTRUCTIONS.has(name) && isToken(instruction)) {
      result.complete = false;
      continue;
    }
    if (instruction.decoded === null) continue;

    if (instruction.programName === "system" && name !== null && LAMPORT_MOVES.has(name)) {
      const [fromName, toName] = LAMPORT_MOVES.get(name) as [string, string];
      const amount = BigInt(stringArg(instruction, "lamports") ?? "0");
      const from = accountNamed(instruction, fromName);
      const to = accountNamed(instruction, toName);
      if (amount === 0n) continue;
      if (from === undefined || to === undefined) {
        result.complete = false;
        continue;
      }
      move(from, to, amount);
      result.nativeTransfers.push({ ...where, kind: name, from, to, lamports: amount.toString() });
    } else if (isToken(instruction) && name === "close_account") {
      const account = accountNamed(instruction, "account");
      const destination = accountNamed(instruction, "destination");
      if (account === undefined || destination === undefined) {
        result.complete = false;
        continue;
      }
      const amount = lamports.get(account) ?? 0n;
      if (amount > 0n) {
        move(account, destination, amount);
        result.nativeTransfers.push({
          ...where,
          kind: "close_account",
          from: account,
          to: destination,
          lamports: amount.toString(),
        });
      }
    } else if (isToken(instruction) && (name === "transfer" || name === "transfer_checked")) {
      const from = accountNamed(instruction, "source");
      const to = accountNamed(instruction, "destination");
      const amountText = stringArg(instruction, "amount");
      if (from === undefined || to === undefined || amountText === undefined) {
        result.complete = false;
        continue;
      }
      const decimalsArg = instruction.decoded.args.decimals;
      const mint =
        accountNamed(instruction, "mint") ?? registry.get(from)?.mint ?? registry.get(to)?.mint ?? null;
      if (mint === null) result.complete = false;
      if (mint === NATIVE_MINT) move(from, to, BigInt(amountText));
      result.tokenTransfers.push({
        ...where,
        kind: name,
        fromTokenAccount: from,
        toTokenAccount: to,
        fromOwner: registry.get(from)?.owner ?? null,
        toOwner: registry.get(to)?.owner ?? null,
        mint,
        amount: amountText,
        decimals:
          typeof decimalsArg === "number"
            ? decimalsArg
            : (registry.get(from)?.decimals ?? registry.get(to)?.decimals ?? null),
      });
    }
  }

  for (const key of input.keys) {
    const expected = (lamports.get(key.address) ?? 0n) - (key.index === 0 ? input.fee : 0n);
    const actual = input.postLamports[key.index] ?? 0n;
    if (expected !== actual) {
      result.unexplained.push({
        index: key.index,
        address: key.address,
        expected: expected.toString(),
        actual: actual.toString(),
        difference: (actual - expected).toString(),
      });
    }
  }
  if (result.unexplained.length > 0) result.complete = false;
  return result;
}
