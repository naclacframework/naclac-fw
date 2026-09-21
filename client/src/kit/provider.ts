import { createSolanaRpc, createSolanaRpcSubscriptions } from "@solana/kit";
import type { TransactionSigner, Commitment, Address } from "@solana/kit";

const _dummyRpc = createSolanaRpc("");
export type NaclacRpc = typeof _dummyRpc;

const _dummyWs = createSolanaRpcSubscriptions("");
export type NaclacRpcSubscriptions = typeof _dummyWs;

/**
 * NaclacProvider — the connection config consumed by every Program instance.
 */
export interface NaclacProvider {
  rpc: NaclacRpc;
  rpcSubscriptions: NaclacRpcSubscriptions;
  signer: TransactionSigner;
  commitment?: Commitment;
  _sendAndConfirm?: (tx: any) => Promise<any>;
  getBalance: (address: string | Address) => Promise<bigint>;
  getTokenBalance: (address: string | Address) => Promise<number>;
  /** Present only on a litesvm-backed provider (see `createLiteSvmProvider`
   * in `./provider-litesvm`) — its presence is how `methods.ts`/`program.ts`
   * detect litesvm mode and take the synchronous-confirm /
   * emulated-event-listener paths instead of the real-cluster ones. */
  litesvm?: import("@naclac-fw/litesvm").LiteSvm;
}
