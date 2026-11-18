/**
 * Types of the raw `getTransaction` result the parser reads (encoding "json",
 * maxSupportedTransactionVersion 1) and of the parsed transaction it returns.
 * Raw field names follow the node's JSON; parsed field names follow the
 * reference fixture, with the additions listed in docs/parser-plan.md.
 */

import type { Serialization } from "./idl.ts";
import type { DecodedValue } from "./layout.ts";

export type TransactionVersion = "legacy" | 0 | 1;

export interface RpcInstruction {
  accounts: number[];
  data: string;
  programIdIndex: number;
  stackHeight?: number | null;
}

export interface RpcInnerInstructionSet {
  index: number;
  instructions: RpcInstruction[];
}

export interface RpcMessageHeader {
  numRequiredSignatures: number;
  numReadonlySignedAccounts: number;
  numReadonlyUnsignedAccounts: number;
}

export interface RpcTransactionConfig {
  computeUnitLimit: number | null;
  heapSize: number | null;
  loadedAccountsDataSizeLimit: number | null;
  priorityFee: number | null;
}

export interface RpcMessage {
  accountKeys: string[];
  header: RpcMessageHeader;
  instructions: RpcInstruction[];
  recentBlockhash: string;
  addressTableLookups?: unknown[];
  transactionConfig?: RpcTransactionConfig;
}

export interface RpcLoadedAddresses {
  writable: string[];
  readonly: string[];
}

export interface RpcTokenBalance {
  accountIndex: number;
  mint: string;
  owner?: string;
  programId?: string;
  uiTokenAmount: { amount: string; decimals: number };
}

export interface RpcMeta {
  err: unknown;
  fee: number;
  computeUnitsConsumed?: number;
  costUnits?: number;
  preBalances: number[];
  postBalances: number[];
  innerInstructions?: RpcInnerInstructionSet[] | null;
  loadedAddresses?: RpcLoadedAddresses | null;
  logMessages?: string[] | null;
  preTokenBalances?: RpcTokenBalance[] | null;
  postTokenBalances?: RpcTokenBalance[] | null;
}

export interface RpcTransactionResult {
  slot: number;
  blockTime: number | null;
  version?: TransactionVersion;
  transactionIndex?: number;
  meta: RpcMeta | null;
  transaction: {
    signatures: string[];
    message: RpcMessage;
  };
}

export type AccountKeySource = "static" | "lookup-writable" | "lookup-readonly";

/**
 * One entry of the transaction's full account list. `signer` and `writable`
 * follow the message header (static keys) or the lookup table section (loaded
 * keys); the runtime may still treat a key the header calls writable as
 * read-only.
 */
export interface AccountKey {
  index: number;
  address: string;
  signer: boolean;
  writable: boolean;
  source: AccountKeySource;
}

export type DecodeStatus =
  | "decoded"
  | "no-idl"
  | "unknown-instruction"
  | "decode-error"
  | "unsupported-type";

export interface DecodedAccount {
  name: string;
  pubkey: string;
  isSigner: boolean;
  isWritable: boolean;
  provided: boolean;
}

/**
 * `trailingBytes` counts data bytes left after the last argument; `missingAccounts`
 * names the IDL accounts the instruction did not supply.
 */
export type ArgValue = DecodedValue | null | readonly ArgValue[];

export interface DecodedInstructionData {
  args: Record<string, ArgValue>;
  accounts: DecodedAccount[];
  missingAccounts: string[];
  trailingBytes: number;
}

export interface ParsedInstruction {
  instructionIndex: number;
  innerInstructionIndex: number | null;
  stackHeight: number | null;
  programId: string;
  rawAccounts: string[];
  rawData: string;
  programName: string | null;
  instructionName: string | null;
  decoded: DecodedInstructionData | null;
  decodeStatus: DecodeStatus;
  decodeReason: string | null;
}

export interface ParsedEvent {
  instructionIndex: number | null;
  innerInstructionIndex: number | null;
  programId: string;
  programName: string;
  name: string;
  discriminator: number[];
  data: Record<string, DecodedValue>;
  trailingBytes: number;
  source: "log";
}

export interface UndecodedLogData {
  instructionIndex: number | null;
  innerInstructionIndex: number | null;
  programId: string;
  reason: string;
  base64: string[];
}

/**
 * The error of a failed transaction, resolved against the IDL of the program
 * that raised it. `kind` is `program` for the program's own errors,
 * `framework` for the naclac framework's, and `unrecognised` otherwise;
 * `accountIndex` and `accountName` are set for framework errors only.
 */
export interface DecodedError {
  instructionIndex: number;
  programId: string | null;
  programName: string | null;
  code: number;
  kind: "program" | "framework" | "unrecognised";
  name: string | null;
  msg: string | null;
  accountIndex: number | null;
  accountName: string | null;
}

export interface ProgramIdlRef {
  programId: string;
  name: string;
  serialization: Serialization;
}

export interface BalanceChange {
  index: number;
  address: string;
  pre: string;
  post: string;
  change: string;
}

/**
 * A token account's balance before and after the transaction, in raw units.
 * `pre` or `post` is null when the account did not exist on that side; `owner`
 * is the owner after the transaction, or before it when the account was closed.
 */
export interface TokenBalanceChange {
  index: number;
  address: string;
  mint: string;
  owner: string | null;
  programId: string | null;
  decimals: number;
  pre: string | null;
  post: string | null;
  change: string;
}

export interface NativeTransfer {
  instructionIndex: number;
  innerInstructionIndex: number | null;
  programId: string;
  kind: string;
  from: string;
  to: string;
  lamports: string;
}

/**
 * An account whose final lamport balance differs from what the listed
 * transfers and the fee explain: a program moved its lamports directly, such
 * as an account closed by its own program. `difference` is actual minus
 * expected.
 */
export interface UnexplainedLamportChange {
  index: number;
  address: string;
  expected: string;
  actual: string;
  difference: string;
}

/** A token transfer; `mint`, `decimals` and the owners are null when the transaction does not reveal them. */
export interface TokenTransfer {
  instructionIndex: number;
  innerInstructionIndex: number | null;
  programId: string;
  kind: "transfer" | "transfer_checked";
  fromTokenAccount: string;
  toTokenAccount: string;
  fromOwner: string | null;
  toOwner: string | null;
  mint: string | null;
  amount: string;
  decimals: number | null;
}

export interface ParsedTransactionData {
  slot: number;
  blockTime: number | null;
  version: TransactionVersion;
  transactionIndex: number | null;
  recentBlockhash: string;
  signatures: string[];
  fee: number;
  feePayer: string;
  transactionStatus: "OK" | "ERROR";
  error: unknown;
  computeUnitsConsumed: number | null;
  costUnits: number | null;
  transactionConfig: RpcTransactionConfig | null;
  accountKeys: AccountKey[];
  logs: string[] | null;
  logsTruncated: boolean | null;
  balanceChanges: BalanceChange[];
  innerInstructionsRecorded: boolean;
  instructions: ParsedInstruction[];
  eventsAvailable: boolean;
  events: ParsedEvent[];
  undecodedLogData: UndecodedLogData[];
  decodedError: DecodedError | null;
  idls: ProgramIdlRef[];
  tokenBalanceChanges: TokenBalanceChange[] | null;
  nativeTransfers: NativeTransfer[];
  tokenTransfers: TokenTransfer[];
  transfersCommitted: boolean;
  transfersComplete: boolean;
  unexplainedLamportChanges: UnexplainedLamportChange[];
}

export interface ParsedTransaction {
  signature: string;
  parserVersion: number;
  parserStatus: "OK";
  parsed: ParsedTransactionData;
}
