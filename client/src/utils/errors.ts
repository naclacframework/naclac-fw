/**
 * Error tables for the Naclac TypeScript client: the Solana runtime errors it
 * names, and the framework errors a Naclac program returns.
 *
 * `NACLAC_FRAMEWORK_ERRORS` mirrors `NaclacError` in naclac-core
 * (`core/src/error.rs`) and the Rust client (`client-rs/src/error.rs`) entry
 * for entry: same code, same name, same message. Keep each entry on one line
 * in the form below; the Rust client's test parses this file and fails when
 * the two tables differ.
 */

/** First code of the framework error range: `10000 + accountIndex * 100 + code`. */
export const FRAMEWORK_ERROR_BASE = 10000;

/** Largest account index a framework error code can name. */
export const MAX_ERROR_ACCOUNT_INDEX = 255;

export const NATIVE_SOLANA_ERRORS: Record<string, string> = {
  "0x0": "Generic Instruction Error / Invalid Account Data",
  "0x1": "Insufficient Funds (Not enough SOL to pay for transaction or rent)",
  "0x2": "Invalid Account Data Format",
  "0x3": "Missing Required Signature",
  "0x4": "Account Data Too Small",
  "0x5": "Insufficient Funds For Rent",
};

export interface NaclacFrameworkError {
  readonly code: number;
  readonly name: string;
  readonly message: string;
}

export const NACLAC_FRAMEWORK_ERRORS: readonly NaclacFrameworkError[] = [
  { code: 1, name: "ConstraintMut", message: "Mutability mismatch" },
  { code: 2, name: "ConstraintSigner", message: "Missing required signature" },
  { code: 3, name: "ConstraintAddress", message: "Address mismatch" },
  { code: 4, name: "ConstraintOwner", message: "Owner mismatch" },
  { code: 5, name: "ConstraintRentExempt", message: "Account is not rent exempt" },
  { code: 6, name: "ConstraintSeeds", message: "PDA derivation failed (Seeds mismatch)" },
  { code: 7, name: "ConstraintExecutable", message: "Account is not executable" },
  { code: 8, name: "ConstraintAccountIsNone", message: "Account is None (Expected initialized account)" },
  { code: 9, name: "ProgramIdMismatch", message: "Program ID mismatch (Missing required program in accounts array)" },
  { code: 10, name: "AccountDataTooSmall", message: "Account data too small (Missing discriminator or insufficient size)" },
  { code: 11, name: "AccountBorrowFailed", message: "Account borrow failed (Already borrowed)" },
  { code: 12, name: "InvalidInstructionData", message: "Invalid instruction data" },
  { code: 13, name: "InsufficientFunds", message: "Insufficient funds for transaction" },
  { code: 14, name: "AccountAlreadyInitialized", message: "Account already initialized" },
  { code: 15, name: "AccountNotInitialized", message: "Account not initialized" },
  { code: 16, name: "NotEnoughAccountKeys", message: "Not enough account keys provided" },
  { code: 17, name: "MaxSeedLengthExceeded", message: "Max seed length exceeded" },
  { code: 18, name: "UnsupportedSysvar", message: "Unsupported sysvar" },
  { code: 19, name: "InvalidRealloc", message: "Invalid reallocation" },
  { code: 20, name: "ArithmeticOverflow", message: "Arithmetic overflow" },
  { code: 21, name: "Unauthorized", message: "Unauthorized access (Authority mismatch)" },
  { code: 22, name: "InvalidAccountDiscriminator", message: "Account type mismatch (Invalid discriminator)" },
  { code: 23, name: "DeserializationFailed", message: "Account deserialization failed" },
  { code: 24, name: "SerializationFailed", message: "Account serialization failed" },
  { code: 25, name: "ConstraintDuplicateMutableAccount", message: "Duplicate mutable account detected in instruction" },
  { code: 26, name: "ConstraintClose", message: "Cannot close account to itself" },
  { code: 27, name: "TooManyCpiSigners", message: "Too many PDA signers in one CPI call (exceeds MAX_CPI_SIGNERS)" },
  { code: 28, name: "TooManyCpiSeeds", message: "Too many seeds for one PDA signer in a CPI call (exceeds MAX_CPI_SEEDS_PER_SIGNER)" },
  { code: 29, name: "TooManyExtraAccounts", message: "Too many extra accounts appended to a CPI (exceeds its fixed maximum)" },
  { code: 30, name: "TooManyCpiAccounts", message: "Too many accounts in one CPI call" },
  { code: 31, name: "TooManyAccountEntries", message: "Instruction carries more than 64 account entries" },
  { code: 32, name: "CapacityExceeded", message: "Fixed-capacity vector or string is full" },
  { code: 33, name: "InvalidUtf8", message: "Fixed-capacity string holds invalid UTF-8" },
];

/** A decoded program error from the IDL. */
export interface NaclacProgramError {
  code: number;
  name: string;
  msg?: string;
}

/**
 * Splits a framework error code (`10000 + accountIndex * 100 + code`) into the
 * error and the index of the account it names. `null` for a code outside the
 * framework range; an in-range code with an unknown error number decodes to
 * `error: undefined`.
 */
export function decodeFrameworkError(
  errorCode: number
): { error: NaclacFrameworkError | undefined; accountIndex: number } | null {
  if (errorCode < FRAMEWORK_ERROR_BASE || errorCode >= FRAMEWORK_ERROR_BASE + (MAX_ERROR_ACCOUNT_INDEX + 1) * 100) {
    return null;
  }
  const offset = errorCode - FRAMEWORK_ERROR_BASE;
  return {
    error: NACLAC_FRAMEWORK_ERRORS.find((e) => e.code === offset % 100),
    accountIndex: Math.floor(offset / 100),
  };
}
