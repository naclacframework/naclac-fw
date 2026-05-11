//! # Naclac Error Framework
//!
//! Provides the core error enumeration used throughout the Naclac framework.
//! These errors are automatically mapped to native `ProgramError::Custom` codes, offset 
//! by a framework-specific base (3000) to avoid collisions with standard errors.

use crate::prelude::ProgramError;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NaclacError {
    ConstraintMut = 1,
    ConstraintSigner = 2,
    ConstraintAddress = 3,
    ConstraintOwner = 4,
    ConstraintRentExempt = 5,
    ConstraintSeeds = 6,
    ConstraintExecutable = 7,
    ConstraintAccountIsNone = 8,
    ConstraintHasOne = 9,
    ProgramIdMismatch = 10,
    AccountDataTooSmall = 11,
    AccountBorrowFailed = 12,
    InvalidInstructionData = 13,
    InsufficientFunds = 14,
    AccountAlreadyInitialized = 15,
    AccountNotInitialized = 16,
    NotEnoughAccountKeys = 17,
    MaxSeedLengthExceeded = 18,
    UnsupportedSysvar = 19,
    InvalidRealloc = 20,
    ArithmeticOverflow = 21,
    Unauthorized = 22,
    InvalidAccountDiscriminator = 23,
    DeserializationFailed = 24,
    SerializationFailed = 25,
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
