import { Connection, PublicKey, Transaction, Keypair } from "@solana/web3.js";

export interface LegacyProvider {
  connection: Connection;
  publicKey?: PublicKey;
  payer?: Keypair;
  signTransaction?: (transaction: Transaction) => Promise<Transaction>;
  signAllTransactions?: (transactions: Transaction[]) => Promise<Transaction[]>;
  commitment?: string;
  getBalance: (address: string | PublicKey) => Promise<bigint>;
  getTokenBalance: (address: string | PublicKey) => Promise<number>;
  /** Present only on a litesvm-backed provider (see `createLiteSvmProvider`
   * in `./provider-litesvm`) — its presence is how `LegacyMethodsBuilder`/
   * `LegacyProgram` detect litesvm mode and take the synchronous-confirm /
   * emulated-event-listener paths instead of the real-cluster ones. */
  litesvm?: import("@naclac-fw/litesvm").LiteSvm;
}
