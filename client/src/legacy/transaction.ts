/**
 * Transaction construction, signing and submission for the legacy
 * (`@solana/web3.js` v3) client. Shared by `LegacyMethodsBuilder`, `transferSol`
 * and the token helpers so every send path builds, signs and confirms
 * transactions identically.
 */

import {
  Transaction,
  TransactionMessage,
  VersionedTransaction,
  type AddressLookupTableAccount,
  type BlockhashWithExpiryBlockHeight,
  type Commitment,
  type Connection,
  type Keypair,
  type PublicKey,
  type TransactionMessageArgs,
  type V1TransactionConfig,
} from "@solana/web3.js";
import type { LegacyProvider } from "./provider";
import { stringifyJson } from "../utils/common";
import {
  V1_DEFAULT_COMPUTE_UNIT_LIMIT,
  V1_DEFAULT_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
} from "../tx-config";

/** A web3.js `TransactionInstruction`, a Kit `Instruction`, or a Kit `InstructionPlan`. */
export type ProviderInstruction = TransactionMessageArgs["instructions"][number];

const COMMITMENT_RANK: Record<Commitment, number> = {
  processed: 0,
  confirmed: 1,
  finalized: 2,
};

export function requireFeePayer(provider: LegacyProvider): PublicKey {
  const feePayer = provider.payer ? provider.payer.publicKey : provider.publicKey;
  if (!feePayer) {
    throw new Error("[Naclac] No fee payer available in the legacy provider.");
  }
  return feePayer;
}

/**
 * Builds the V1 message-level resource config. Both limits are always set
 * (see `V1_DEFAULT_*` in `tx-config.ts`); the priority fee is the total
 * lamports for the whole transaction and is set only when given.
 */
export function v1ConfigFor(limits: {
  computeUnitLimit?: number;
  loadedAccountsDataSizeLimit?: number;
  priorityFee?: number | bigint;
}): V1TransactionConfig {
  const config: V1TransactionConfig = {
    computeUnitLimit: limits.computeUnitLimit ?? V1_DEFAULT_COMPUTE_UNIT_LIMIT,
    loadedAccountsDataSizeLimit:
      limits.loadedAccountsDataSizeLimit ?? V1_DEFAULT_LOADED_ACCOUNTS_DATA_SIZE_LIMIT,
  };
  if (limits.priorityFee !== undefined) {
    config.priorityFeeLamports = BigInt(limits.priorityFee);
  }
  return config;
}

/**
 * Compiles `instructions` into an unsigned transaction of the requested wire
 * version, with `latestBlockhash` and `feePayer` set. `addressLookupTableAccounts`
 * only applies to V0; `v1Config` only applies to V1.
 */
export function compileProviderTransaction(args: {
  instructions: ProviderInstruction[];
  feePayer: PublicKey;
  latestBlockhash: BlockhashWithExpiryBlockHeight;
  txVersion: 0 | 1 | "legacy";
  v1Config: V1TransactionConfig;
  addressLookupTableAccounts?: AddressLookupTableAccount[];
}): Transaction | VersionedTransaction {
  const { instructions, feePayer, latestBlockhash, txVersion } = args;

  if (txVersion === "legacy") {
    const tx = new Transaction();
    for (const ix of instructions) {
      tx.add(ix);
    }
    tx.recentBlockhash = latestBlockhash.blockhash;
    tx.feePayer = feePayer;
    return tx;
  }

  const message = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: latestBlockhash.blockhash,
    instructions,
  });
  if (txVersion === 1) {
    return new VersionedTransaction(message.compileToV1Message(args.v1Config));
  }
  return new VersionedTransaction(message.compileToV0Message(args.addressLookupTableAccounts));
}

/**
 * Signs with the provider's payer and `extraSigners`, then passes the result
 * through the provider's wallet `signTransaction` when it has one.
 */
export async function signProviderTransaction(
  provider: LegacyProvider,
  tx: Transaction | VersionedTransaction,
  extraSigners: Keypair[],
): Promise<Transaction | VersionedTransaction> {
  const signers: Keypair[] = [];
  const seen = new Set<string>();
  for (const signer of provider.payer ? [provider.payer, ...extraSigners] : extraSigners) {
    if (seen.has(signer.address)) continue;
    seen.add(signer.address);
    signers.push(signer);
  }

  if (signers.length > 0) {
    if (tx instanceof VersionedTransaction) {
      await tx.sign(signers);
    } else {
      await tx.partialSign(...signers);
    }
  }

  return provider.signTransaction ? await provider.signTransaction(tx) : tx;
}

/**
 * Polls `getSignatureStatuses` over plain HTTP until `signature` reaches
 * `commitment`, fails on-chain, or `lastValidBlockHeight` passes.
 */
export async function pollForConfirmation(
  connection: Connection,
  signature: string,
  lastValidBlockHeight: bigint,
  commitment: Commitment,
): Promise<void> {
  const targetRank = COMMITMENT_RANK[commitment];
  if (targetRank === undefined) {
    throw new Error(
      `[Naclac] Unknown commitment ${JSON.stringify(commitment)}; use "processed", "confirmed" or "finalized".`,
    );
  }
  const pollIntervalMs = 400;

  for (;;) {
    const { value: statuses } = await connection.getSignatureStatuses([signature]);
    const status = statuses[0];

    if (status?.err) {
      throw new Error(`[Naclac] Transaction failed on-chain: ${stringifyJson(status.err)}`);
    }

    const statusRank = status?.confirmationStatus ? COMMITMENT_RANK[status.confirmationStatus] : -1;
    if (statusRank >= targetRank) return;

    const blockHeight = await connection.getBlockHeight(commitment);
    if (blockHeight > lastValidBlockHeight) {
      throw new Error(
        `[Naclac] Transaction confirmation timed out: blockhash expired at height ${lastValidBlockHeight}.`,
      );
    }

    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
}

/**
 * Sends an already-signed transaction and, on a real cluster, waits for the
 * provider's commitment. A litesvm-backed provider executes `sendRawTransaction`
 * synchronously, so there is nothing to wait for.
 */
export async function sendAndConfirmSigned(
  provider: LegacyProvider,
  signedTx: Transaction | VersionedTransaction,
  lastValidBlockHeight: bigint,
): Promise<string> {
  const rawTx = await signedTx.serialize();
  const signature = await provider.connection.sendRawTransaction(rawTx, {
    skipPreflight: false,
    preflightCommitment: provider.commitment,
  });
  if (!provider.litesvm) {
    await pollForConfirmation(provider.connection, signature, lastValidBlockHeight, provider.commitment);
  }
  return signature;
}

/**
 * Builds, signs, sends and confirms a transaction made of `instructions` in
 * the given wire version.
 */
export async function sendInstructions(
  provider: LegacyProvider,
  instructions: ProviderInstruction[],
  extraSigners: Keypair[],
  txVersion: 0 | 1 | "legacy",
): Promise<string> {
  const latestBlockhash = await provider.connection.getLatestBlockhash(provider.commitment);
  const tx = compileProviderTransaction({
    instructions,
    feePayer: requireFeePayer(provider),
    latestBlockhash,
    txVersion,
    v1Config: v1ConfigFor({}),
  });
  const signedTx = await signProviderTransaction(provider, tx, extraSigners);
  return sendAndConfirmSigned(provider, signedTx, latestBlockhash.lastValidBlockHeight);
}
