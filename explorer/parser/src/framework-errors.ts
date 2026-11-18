/**
 * Names of the naclac framework's own error types. A framework error code is
 * `FRAMEWORK_ERROR_BASE + account_index * 100 + type`; a program's own errors
 * start at `USER_ERROR_BASE`. The table must match `NaclacError` in
 * core/src/error.rs, which a test checks.
 */

export const USER_ERROR_BASE = 6000;
export const FRAMEWORK_ERROR_BASE = 10_000;

export const FRAMEWORK_ERRORS: ReadonlyMap<number, string> = new Map([
  [1, "ConstraintMut"],
  [2, "ConstraintSigner"],
  [3, "ConstraintAddress"],
  [4, "ConstraintOwner"],
  [5, "ConstraintRentExempt"],
  [6, "ConstraintSeeds"],
  [7, "ConstraintExecutable"],
  [8, "ConstraintAccountIsNone"],
  [9, "ProgramIdMismatch"],
  [10, "AccountDataTooSmall"],
  [11, "AccountBorrowFailed"],
  [12, "InvalidInstructionData"],
  [13, "InsufficientFunds"],
  [14, "AccountAlreadyInitialized"],
  [15, "AccountNotInitialized"],
  [16, "NotEnoughAccountKeys"],
  [17, "MaxSeedLengthExceeded"],
  [18, "UnsupportedSysvar"],
  [19, "InvalidRealloc"],
  [20, "ArithmeticOverflow"],
  [21, "Unauthorized"],
  [22, "InvalidAccountDiscriminator"],
  [23, "DeserializationFailed"],
  [24, "SerializationFailed"],
  [25, "ConstraintDuplicateMutableAccount"],
  [26, "ConstraintClose"],
  [29, "TooManyExtraAccounts"],
  [30, "TooManyCpiAccounts"],
  [31, "TooManyAccountEntries"],
  [32, "CapacityExceeded"],
  [33, "InvalidUtf8"],
  [34, "ConstraintDuplicateAccount"],
  [35, "ConstraintTokenMint"],
  [36, "ConstraintTokenAuthority"],
  [37, "ConstraintMintDecimals"],
  [38, "ConstraintMintAuthority"],
  [39, "ConstraintMintFreezeAuthority"],
]);
