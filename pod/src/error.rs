//! The errors `naclac-pod`'s fallible operations return, and their conversion
//! into a program error.
//!
//! The codes are framework error codes at account index 0
//! (`10_000 + index * 100 + number`, see `naclac-core`'s `NaclacError`), and
//! `naclac-core` asserts at compile time that they equal the matching
//! `NaclacError` variants. They live here because `naclac-core` cannot
//! implement `From<PodError>` for a `ProgramError` that is neither its own type
//! nor `PodError`.

/// `NaclacError::InvalidInstructionData.to_code(0)`.
pub const INVALID_INSTRUCTION_DATA_CODE: u32 = 10_012;

/// `NaclacError::CapacityExceeded.to_code(0)`.
pub const CAPACITY_EXCEEDED_CODE: u32 = 10_032;

/// `NaclacError::InvalidUtf8.to_code(0)`.
pub const INVALID_UTF8_CODE: u32 = 10_033;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, kani, feature = "debug"), derive(Debug))]
pub enum PodError {
    /// A list or string has no room for what was added.
    CapacityExceeded,
    /// A string's bytes are not valid UTF-8.
    InvalidUtf8,
    /// Instruction-data bytes do not form the requested view: a length that is
    /// not a whole number of elements, or text that is not valid UTF-8.
    InvalidInstructionData,
    /// Stored enum bytes name no variant, or hold an invalid nested value.
    InvalidEnumValue,
}

impl PodError {
    /// The framework error code, or `None` for an error that maps to a
    /// built-in program error.
    pub const fn custom_code(self) -> Option<u32> {
        match self {
            PodError::CapacityExceeded => Some(CAPACITY_EXCEEDED_CODE),
            PodError::InvalidUtf8 => Some(INVALID_UTF8_CODE),
            PodError::InvalidInstructionData => Some(INVALID_INSTRUCTION_DATA_CODE),
            PodError::InvalidEnumValue => None,
        }
    }
}

#[cfg(any(feature = "solana", feature = "pinocchio"))]
impl From<PodError> for solana_program_error::ProgramError {
    fn from(error: PodError) -> Self {
        match error.custom_code() {
            Some(code) => Self::Custom(code),
            None => Self::InvalidAccountData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_framework_codes_at_account_index_zero() {
        assert_eq!(PodError::InvalidInstructionData.custom_code(), Some(10_012));
        assert_eq!(PodError::CapacityExceeded.custom_code(), Some(10_032));
        assert_eq!(PodError::InvalidUtf8.custom_code(), Some(10_033));
        assert_eq!(PodError::InvalidEnumValue.custom_code(), None);
    }

    #[cfg(feature = "solana")]
    #[test]
    fn converts_to_the_solana_program_error() {
        use solana_program_error::ProgramError;
        assert_eq!(
            ProgramError::from(PodError::CapacityExceeded),
            ProgramError::Custom(10_032)
        );
        assert_eq!(
            ProgramError::from(PodError::InvalidEnumValue),
            ProgramError::InvalidAccountData
        );
    }
}
