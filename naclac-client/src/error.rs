use solana_program::instruction::InstructionError;
use thiserror::Error;

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
}

impl NaclacError {
    pub fn description(&self) -> &'static str {
        match self {
            NaclacError::ConstraintMut => "Account must be mutable",
            NaclacError::ConstraintSigner => "Account must be a signer",
            NaclacError::ConstraintAddress => "Account address constraint mismatch",
            NaclacError::ConstraintOwner => "Account owner constraint mismatch",
            NaclacError::ConstraintRentExempt => "Account is not rent exempt",
            NaclacError::ConstraintSeeds => "Account seeds derivation constraint mismatch",
            NaclacError::ConstraintExecutable => "Account must be executable (program)",
            NaclacError::ConstraintAccountIsNone => "Account is required but was not provided",
            NaclacError::ProgramIdMismatch => "Program ID mismatch",
            NaclacError::AccountDataTooSmall => "Account data size is too small",
            NaclacError::AccountBorrowFailed => "Account borrow failed (concurrent borrow overlap)",
            NaclacError::InvalidInstructionData => "Invalid instruction data layout",
            NaclacError::InsufficientFunds => "Insufficient funds in signer account",
            NaclacError::AccountAlreadyInitialized => "Account is already initialized",
            NaclacError::AccountNotInitialized => "Account has not been initialized",
            NaclacError::NotEnoughAccountKeys => "Not enough account keys provided for instruction",
            NaclacError::MaxSeedLengthExceeded => "PDA seed length exceeded maximum",
            NaclacError::UnsupportedSysvar => "Unsupported sysvar account type",
            NaclacError::InvalidRealloc => "Invalid account data reallocation",
            NaclacError::ArithmeticOverflow => "Safe math arithmetic overflow occurred",
            NaclacError::Unauthorized => "User is unauthorized to perform this operation",
            NaclacError::InvalidAccountDiscriminator => "Invalid account discriminator prefix",
            NaclacError::DeserializationFailed => "Borsh/Zero-Copy deserialization failed",
            NaclacError::SerializationFailed => "Borsh/Zero-Copy serialization failed",
            NaclacError::ConstraintDuplicateMutableAccount => {
                "Duplicate mutable account detected in instruction"
            }
            NaclacError::ConstraintClose => "Cannot close account to itself",
            NaclacError::TooManyCpiSigners => {
                "Too many PDA signers in one CPI call (exceeds MAX_CPI_SIGNERS)"
            }
            NaclacError::TooManyCpiSeeds => {
                "Too many seeds for one PDA signer in a CPI call (exceeds MAX_CPI_SEEDS_PER_SIGNER)"
            }
        }
    }
}

pub fn decode_custom_error(code: u32) -> Option<(NaclacError, usize)> {
    if !(3000..6000).contains(&code) {
        return None;
    }
    let offset_code = code - 3000;
    let index = (offset_code / 100) as usize;
    let error_val = offset_code % 100;

    let error = match error_val {
        1 => NaclacError::ConstraintMut,
        2 => NaclacError::ConstraintSigner,
        3 => NaclacError::ConstraintAddress,
        4 => NaclacError::ConstraintOwner,
        5 => NaclacError::ConstraintRentExempt,
        6 => NaclacError::ConstraintSeeds,
        7 => NaclacError::ConstraintExecutable,
        8 => NaclacError::ConstraintAccountIsNone,
        9 => NaclacError::ProgramIdMismatch,
        10 => NaclacError::AccountDataTooSmall,
        11 => NaclacError::AccountBorrowFailed,
        12 => NaclacError::InvalidInstructionData,
        13 => NaclacError::InsufficientFunds,
        14 => NaclacError::AccountAlreadyInitialized,
        15 => NaclacError::AccountNotInitialized,
        16 => NaclacError::NotEnoughAccountKeys,
        17 => NaclacError::MaxSeedLengthExceeded,
        18 => NaclacError::UnsupportedSysvar,
        19 => NaclacError::InvalidRealloc,
        20 => NaclacError::ArithmeticOverflow,
        21 => NaclacError::Unauthorized,
        22 => NaclacError::InvalidAccountDiscriminator,
        23 => NaclacError::DeserializationFailed,
        24 => NaclacError::SerializationFailed,
        25 => NaclacError::ConstraintDuplicateMutableAccount,
        26 => NaclacError::ConstraintClose,
        27 => NaclacError::TooManyCpiSigners,
        28 => NaclacError::TooManyCpiSeeds,
        _ => return None,
    };
    Some((error, index))
}

#[derive(Error, Debug)]
pub enum NaclacClientError {
    #[error("Solana RPC Error: {0}")]
    RpcError(String),

    #[error("LiteSVM Execution Error: {0}")]
    LiteSvmError(String),

    #[error("Transaction failed: {instruction_err:?} (logs: {logs:?})")]
    TransactionFailed {
        instruction_err: InstructionError,
        logs: Vec<String>,
        translated_msg: String,
    },

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("General Client Error: {0}")]
    General(String),
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `decode_custom_error` never panics for any `u32` — the one
    /// subtraction (`code - 3000`) is guarded by the range check
    /// immediately before it, so this also confirms that guard is actually
    /// sufficient rather than just assumed to be.
    #[kani::proof]
    fn prove_decode_custom_error_never_panics() {
        let code: u32 = kani::any();
        let _ = decode_custom_error(code);
    }

    /// Correctness: whenever `decode_custom_error` returns `Some((error,
    /// index))`, reconstructing the code from `error`/`index` via the same
    /// formula `NaclacError::to_code` (naclac-core's encoder) uses must
    /// recover the exact original `code` — proving this client-side decoder
    /// is a true inverse of the on-chain encoder, not just a plausible-
    /// looking approximation.
    #[kani::proof]
    fn prove_decode_custom_error_is_correct_inverse() {
        let code: u32 = kani::any();
        if let Some((error, index)) = decode_custom_error(code) {
            let reconstructed = 3000 + (index as u32) * 100 + (error as u32);
            assert_eq!(reconstructed, code, "decode must be a true inverse of the encoder");
        }
    }
}

/// Depth of the last "Program X invoke [N]" line in `logs` — the stack depth
/// at which the actual failure most likely occurred. `1` means the top-level
/// submitted instruction; anything deeper means the failure originated
/// inside a CPI target, whose own account list `account_names` (always the
/// top-level instruction's) cannot correctly describe — a valid-looking
/// index there names an unrelated account from the wrong instruction.
fn last_invoke_depth(logs: &[String]) -> Option<u32> {
    logs.iter().rev().find_map(|line| {
        let start = line.find("invoke [")? + "invoke [".len();
        let end = line[start..].find(']')?;
        line[start..start + end].parse::<u32>().ok()
    })
}

pub fn translate_error_code(code: u32, account_names: Option<&[&str]>, logs: &[String]) -> String {
    if let Some((error, index)) = decode_custom_error(code) {
        let is_top_level = last_invoke_depth(logs).map(|d| d == 1).unwrap_or(true);
        let account_desc = if !is_top_level {
            format!(
                "Account Index: {} (inside a nested CPI — the top-level instruction's own \
                 account list can't name it; check the program logs for the real account)",
                index
            )
        } else if let Some(names) = account_names {
            if index < names.len() {
                format!("Account: '{}' (Index {})", names[index], index)
            } else {
                format!("Account Index: {}", index)
            }
        } else {
            format!("Account Index: {}", index)
        };

        format!(
            "❌ Naclac Security Constraint Violated!\n\
             Error Type:  {:?} ({})\n\
             {}\n\
             Description: {}",
            error,
            code,
            account_desc,
            error.description()
        )
    } else {
        format!("Custom program error: {}", code)
    }
}
