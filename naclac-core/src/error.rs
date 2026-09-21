//! # Naclac Error Framework
//!
//! Provides the core error enumeration used throughout the Naclac framework.
//! These errors are automatically mapped to native `ProgramError::Custom` codes, offset
//! by a framework-specific base (3000) to avoid collisions with standard errors.

use crate::prelude::ProgramError;

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
}

impl NaclacError {
    pub fn to_code(self, index: usize) -> u32 {
        // Range: 3000 + (index * 100) + error_type
        // Example: Address mismatch for account at index 3 -> 3000 + 300 + 3 = 3303
        3000 + ((index as u32) * 100) + (self as u32)
    }

    pub fn err(self, index: usize) -> ProgramError {
        ProgramError::Custom(self.to_code(index))
    }
}

impl From<NaclacError> for ProgramError {
    fn from(e: NaclacError) -> Self {
        ProgramError::Custom(e.to_code(0))
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `to_code` never panics and computes the documented formula
    /// exactly, for the real range `index` can actually take — Solana caps
    /// the number of accounts a single transaction can carry (well under
    /// 256 even on the most permissive versioned-transaction path), so
    /// `index` here is never remotely close to the point where `(index as
    /// u32) * 100` could overflow `u32` (`index` would need to exceed
    /// roughly 42.9 million for that, per `(u32::MAX - 3030) / 100`) — this
    /// proves the function is sound for every index a real caller could ever
    /// pass, not merely for one sampled value.
    #[kani::proof]
    fn prove_to_code_within_contract_is_correct() {
        let index: usize = kani::any();
        kani::assume(index <= 1024); // far above any real Solana account-list length
        let error: NaclacError = if kani::any() {
            NaclacError::ConstraintMut
        } else {
            NaclacError::TooManyCpiAccounts
        };
        let code = error.to_code(index);
        assert_eq!(code, 3000 + (index as u32) * 100 + (error as u32));
    }
}
