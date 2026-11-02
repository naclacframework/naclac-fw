import { Connection, PublicKey, Transaction, VersionedTransaction, Keypair, type Commitment } from "@solana/web3.js";
import { type TxVersion, normalizeTxVersion } from "../tx-config";

export { type TxVersion, normalizeTxVersion };

export interface LegacyProvider {
  connection: Connection;
  publicKey?: PublicKey;
  payer?: Keypair;
  signTransaction?: <T extends Transaction | VersionedTransaction>(transaction: T) => Promise<T>;
  signAllTransactions?: <T extends Transaction | VersionedTransaction>(transactions: T[]) => Promise<T[]>;
  commitment: Commitment;
  defaultTxVersion?: TxVersion;
  getBalance: (address: string | PublicKey) => Promise<bigint>;
  getTokenBalance: (address: string | PublicKey) => Promise<number>;
  /** Present only on a litesvm-backed provider (see `createLiteSvmProvider`
   * in `./provider-litesvm`) — its presence is how `LegacyMethodsBuilder`/
   * `LegacyProgram` detect litesvm mode and take the synchronous-confirm /
   * emulated-event-listener paths instead of the real-cluster ones. */
  litesvm?: import("@naclac-fw/litesvm").LiteSvm;
}
