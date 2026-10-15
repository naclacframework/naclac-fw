import type {
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  TransactionSigner,
  Commitment,
  Address,
} from "@solana/kit";

export type NaclacRpc = ReturnType<typeof createSolanaRpc>;
export type NaclacRpcSubscriptions = ReturnType<typeof createSolanaRpcSubscriptions>;

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

/**
 * NaclacProvider — the connection config consumed by every Program instance.
 */
export interface NaclacProvider {
  rpc: NaclacRpc;
  rpcSubscriptions: NaclacRpcSubscriptions;
  signer: TransactionSigner;
  commitment?: Commitment;
  defaultTxVersion?: TxVersion;
  _sendAndConfirm?: (tx: any) => Promise<any>;
  getBalance: (address: string | Address) => Promise<bigint>;
  getTokenBalance: (address: string | Address) => Promise<number>;
  /** Present only on a litesvm-backed provider (see `createLiteSvmProvider`
   * in `./provider-litesvm`) — its presence is how `methods.ts`/`program.ts`
   * detect litesvm mode and take the synchronous-confirm /
   * emulated-event-listener paths instead of the real-cluster ones. */
  litesvm?: import("@naclac-fw/litesvm").LiteSvm;
}
