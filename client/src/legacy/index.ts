export * from "../index";
export {
  LegacyProgram,
  LegacyMethodsBuilder,
  LegacyAccountFetcher,
} from "./program";
export type { LegacyProvider } from "./provider";
export * from "./token";

// ─── Re-export all @solana/web3.js primitives needed by generated client code ───
// Generated legacy SDKs must ONLY import from @naclac-fw/client/legacy — never
// directly from @solana/web3.js. All required primitives are surfaced here.
export {
  // Core types — addresses and signers
  PublicKey,
  Keypair,
  // Connection and transactions
  Connection,
  Transaction,
  TransactionInstruction,
  // Data types used in account fetch results
  type AccountInfo,
  type ParsedAccountData,
  // RPC config options
  type ConfirmOptions,
  type SendOptions,
  type GetProgramAccountsFilter,
  type Commitment,
} from "@solana/web3.js";

export { createProvider, loadNodeWallet, transferSol } from "./setup";
export { createLiteSvmProvider, type LiteSvmLegacyProvider } from "./provider-litesvm";
