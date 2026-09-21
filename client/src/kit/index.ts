export * from "../index";
export { Program } from "./program";
export { MethodsBuilder } from "./methods";
export { AccountFetcher, fetchAllAccountsByProgram } from "./account";
export type { NaclacProvider } from "./provider";
export { createProvider, loadNodeWallet, transferSol } from "./setup";
export { createLiteSvmProvider, type LiteSvmProvider } from "./provider-litesvm";
export * from "./token";

// ─── Re-export all @solana/kit primitives needed by generated client code ───
// Generated SDKs must ONLY import from @naclac-fw/client/kit — never directly
// from @solana/kit. All required primitives are surfaced here.
export {
  // Address utilities
  address,
  createKeyPairSignerFromBytes,
  generateKeyPairSigner,
  // PDA derivation — required for generated PDA helper methods
  getProgramDerivedAddress,
  // Address codec — used by generated account decoders
  getAddressEncoder,
  getAddressDecoder,
  // Transaction pipeline helpers
  pipe,
  // Enums and constants
  AccountRole,
  // Account fetch helpers — used by generated fetchXxx() functions
  fetchEncodedAccount,
  fetchEncodedAccounts,
  assertAccountExists,
  assertAccountsExist,
  decodeAccount,
  // Struct codecs — foundation of generated getXxxDecoder()
  getStructEncoder,
  getStructDecoder,
  // Numeric codecs
  getU8Encoder,   getU8Decoder,
  getU16Encoder,  getU16Decoder,
  getU32Encoder,  getU32Decoder,
  getU64Encoder,  getU64Decoder,
  getU128Encoder, getU128Decoder,
  getI8Encoder,   getI8Decoder,
  getI16Encoder,  getI16Decoder,
  getI32Encoder,  getI32Decoder,
  getI64Encoder,  getI64Decoder,
  getI128Encoder, getI128Decoder,
  // Boolean codec
  getBooleanEncoder, getBooleanDecoder,
  // Bytes/string codecs
  getBytesEncoder, getBytesDecoder,
  getUtf8Encoder,  getUtf8Decoder,
  // Option / array codecs
  getOptionEncoder, getOptionDecoder,
  getArrayEncoder,  getArrayDecoder,
  // Codec combinator
  combineCodec,
  // Types
  type Address,
  type KeyPairSigner,
  type TransactionSigner,
  type ProgramDerivedAddress,
  type Commitment,
  type Signature,
  type EncodedAccount,
  type MaybeEncodedAccount,
  type Account,
  type MaybeAccount,
  type Encoder,
  type Decoder,
  type Codec,
  type FixedSizeEncoder,
  type FixedSizeDecoder,
  type FixedSizeCodec,
  type FetchAccountConfig,
  type FetchAccountsConfig,
  // Lamports
  lamports,
  type Lamports,
} from "@solana/kit";


import { generateKeyPairSigner as genKeyPairSigner, type KeyPairSigner } from "@solana/kit";

/**
 * Generates an extractable KeyPairSigner. By default @solana/kit generates
 * non-extractable keys; this helper enables extraction for testing and key
 * export scenarios.
 */
export async function generateExtractableKeyPairSigner(): Promise<KeyPairSigner> {
  return genKeyPairSigner(true);
}

import { getBase58Codec } from "@solana/codecs";
export { getBase58Codec };

import { address as toAddress } from "@solana/kit";
import {
  SYSTEM_PROGRAM_ID as SYSTEM_PROGRAM_ID_STR,
  TOKEN_PROGRAM_ID as TOKEN_PROGRAM_ID_STR,
  TOKEN_2022_PROGRAM_ID as TOKEN_2022_PROGRAM_ID_STR,
  ATA_PROGRAM_ID as ATA_PROGRAM_ID_STR,
  SYSVAR_RENT_PUBKEY as SYSVAR_RENT_PUBKEY_STR,
} from "../constants";

/** Well-known program addresses — available as typed `Address` values. */
export const SYSTEM_PROGRAM_ID = toAddress(SYSTEM_PROGRAM_ID_STR);
export const TOKEN_PROGRAM_ID = toAddress(TOKEN_PROGRAM_ID_STR);
export const TOKEN_2022_PROGRAM_ID = toAddress(TOKEN_2022_PROGRAM_ID_STR);
export const ATA_PROGRAM_ID = toAddress(ATA_PROGRAM_ID_STR);
export const SYSVAR_RENT_PUBKEY = toAddress(SYSVAR_RENT_PUBKEY_STR);

