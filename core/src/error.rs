//! # Naclac Error Framework
//!
//! Provides the core error enumeration used throughout the Naclac framework.
//! These errors are automatically mapped to native `ProgramError::Custom` codes
//! `FRAMEWORK_ERROR_BASE + account_index * 100 + error_type`.

use crate::prelude::ProgramError;

/// First code `#[error_code]` assigns to a user error variant.
pub const USER_ERROR_BASE: u32 = 6000;

/// Base of every framework error code. Every user error code
/// (`USER_ERROR_BASE + discriminant`) must stay below this, or a user error
/// becomes indistinguishable from a framework error; `#[error_code]`
/// enforces that at compile time.
pub const FRAMEWORK_ERROR_BASE: u32 = 10_000;

#[cfg_attr(feature = "debug-mode", derive(Debug))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum NaclacError {
    ConstraintMut = 1,
    ConstraintSigner = 2,
    ConstraintAddress = 3,
    ConstraintOwner = 4,
    ConstraintRentExempt = 5,
    ConstraintSeeds = 6,
    ConstraintExecutable = 7,
    ConstraintAccountIsNone = 8,
    ProgramIdMismatch = 9,
    AccountDataTooSmall = 10,
    AccountBorrowFailed = 11,
    InvalidInstructionData = 12,
    InsufficientFunds = 13,
    AccountAlreadyInitialized = 14,
    AccountNotInitialized = 15,
    NotEnoughAccountKeys = 16,
    MaxSeedLengthExceeded = 17,
    UnsupportedSysvar = 18,
    InvalidRealloc = 19,
    ArithmeticOverflow = 20,
    Unauthorized = 21,
    InvalidAccountDiscriminator = 22,
    DeserializationFailed = 23,
    SerializationFailed = 24,
    ConstraintDuplicateMutableAccount = 25,
    ConstraintClose = 26,
    TooManyCpiSigners = 27,
    TooManyCpiSeeds = 28,
    TooManyExtraAccounts = 29,
    TooManyCpiAccounts = 30,
    TooManyAccountEntries = 31,
    CapacityExceeded = 32,
    InvalidUtf8 = 33,
}

/// Largest account index a framework error code can name. The runtime
/// allows at most 255 accounts per instruction, and `naclac-client` decodes
/// indices up to this value.
pub const MAX_ERROR_ACCOUNT_INDEX: usize = 255;

impl NaclacError {
    /// `FRAMEWORK_ERROR_BASE + index * 100 + error_type`, with `index`
    /// clamped to [`MAX_ERROR_ACCOUNT_INDEX`] so the arithmetic cannot
    /// overflow.
    pub const fn to_code(self, index: usize) -> u32 {
        let index = if index > MAX_ERROR_ACCOUNT_INDEX {
            MAX_ERROR_ACCOUNT_INDEX
        } else {
            index
        };
        FRAMEWORK_ERROR_BASE + ((index as u32) * 100) + (self as u32)
    }

    pub fn err(self, index: usize) -> ProgramError {
        ProgramError::Custom(self.to_code(index))
    }
}

// The codes `naclac-pod` converts its errors to must be these variants' codes.
#[cfg(feature = "bytemuck")]
const _: () = {
    assert!(
        NaclacError::InvalidInstructionData.to_code(0) == naclac_pod::INVALID_INSTRUCTION_DATA_CODE
    );
    assert!(NaclacError::CapacityExceeded.to_code(0) == naclac_pod::CAPACITY_EXCEEDED_CODE);
    assert!(NaclacError::InvalidUtf8.to_code(0) == naclac_pod::INVALID_UTF8_CODE);
};

impl From<NaclacError> for ProgramError {
    fn from(e: NaclacError) -> Self {
        ProgramError::Custom(e.to_code(0))
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `to_code` never panics for any `index`, and that its result
    /// is the documented formula on the clamped index and always lies
    /// inside the range `naclac-client` decodes as a framework error.
    #[kani::proof]
    fn prove_to_code_is_total_and_in_decoder_range() {
        let index: usize = kani::any();
        let error: NaclacError = if kani::any() {
            NaclacError::ConstraintMut
        } else {
            NaclacError::TooManyAccountEntries
        };
        let code = error.to_code(index);
        let clamped = if index > MAX_ERROR_ACCOUNT_INDEX {
            MAX_ERROR_ACCOUNT_INDEX
        } else {
            index
        };
        assert_eq!(code, 10_000 + (clamped as u32) * 100 + (error as u32));
        assert!(code >= FRAMEWORK_ERROR_BASE && code < FRAMEWORK_ERROR_BASE + 256 * 100);
    }
}
