import { Connection, PublicKey, Transaction, VersionedTransaction, Keypair } from "@solana/web3.js";

export type TxVersion = "v0" | "v1" | "legacy";

/**
 * Maps a transaction version to its wire form. `undefined` means the default
 * (V1); any value other than "v0", "v1" or "legacy" throws rather than
 * silently building a different transaction version than the caller asked for.
 */
export function normalizeTxVersion(version?: TxVersion): 0 | 1 | "legacy" {
  if (version === undefined || version === "v1") return 1;
  if (version === "v0") return 0;
  if (version === "legacy") return "legacy";
  throw new Error(
    `[Naclac] Unknown transaction version ${JSON.stringify(version)}; use "v0", "v1" or "legacy".`,
  );
}

export interface LegacyProvider {
  connection: Connection;
  publicKey?: PublicKey;
  payer?: Keypair;
  signTransaction?: <T extends Transaction | VersionedTransaction>(transaction: T) => Promise<T>;
  signAllTransactions?: <T extends Transaction | VersionedTransaction>(transactions: T[]) => Promise<T[]>;
  commitment?: string;
  defaultTxVersion?: TxVersion;
  getBalance: (address: string | PublicKey) => Promise<bigint>;
  getTokenBalance: (address: string | PublicKey) => Promise<number>;
  /** Present only on a litesvm-backed provider (see `createLiteSvmProvider`
   * in `./provider-litesvm`) — its presence is how `LegacyMethodsBuilder`/
   * `LegacyProgram` detect litesvm mode and take the synchronous-confirm /
   * emulated-event-listener paths instead of the real-cluster ones. */
  litesvm?: import("@naclac-fw/litesvm").LiteSvm;
}
