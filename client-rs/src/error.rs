use {
    naclac_core::error::FRAMEWORK_ERROR_BASE, solana_instruction_error::InstructionError,
    thiserror::Error,
};

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
    TooManyExtraAccounts = 29,
    TooManyCpiAccounts = 30,
    TooManyAccountEntries = 31,
    CapacityExceeded = 32,
    InvalidUtf8 = 33,
}

impl NaclacError {
    /// Every framework error, in code order.
    pub const ALL: [NaclacError; 33] = [
        NaclacError::ConstraintMut,
        NaclacError::ConstraintSigner,
        NaclacError::ConstraintAddress,
        NaclacError::ConstraintOwner,
        NaclacError::ConstraintRentExempt,
        NaclacError::ConstraintSeeds,
        NaclacError::ConstraintExecutable,
        NaclacError::ConstraintAccountIsNone,
        NaclacError::ProgramIdMismatch,
        NaclacError::AccountDataTooSmall,
        NaclacError::AccountBorrowFailed,
        NaclacError::InvalidInstructionData,
        NaclacError::InsufficientFunds,
        NaclacError::AccountAlreadyInitialized,
        NaclacError::AccountNotInitialized,
        NaclacError::NotEnoughAccountKeys,
        NaclacError::MaxSeedLengthExceeded,
        NaclacError::UnsupportedSysvar,
        NaclacError::InvalidRealloc,
        NaclacError::ArithmeticOverflow,
        NaclacError::Unauthorized,
        NaclacError::InvalidAccountDiscriminator,
        NaclacError::DeserializationFailed,
        NaclacError::SerializationFailed,
        NaclacError::ConstraintDuplicateMutableAccount,
        NaclacError::ConstraintClose,
        NaclacError::TooManyCpiSigners,
        NaclacError::TooManyCpiSeeds,
        NaclacError::TooManyExtraAccounts,
        NaclacError::TooManyCpiAccounts,
        NaclacError::TooManyAccountEntries,
        NaclacError::CapacityExceeded,
        NaclacError::InvalidUtf8,
    ];

    pub fn description(&self) -> &'static str {
        match self {
            NaclacError::ConstraintMut => "Mutability mismatch",
            NaclacError::ConstraintSigner => "Missing required signature",
            NaclacError::ConstraintAddress => "Address mismatch",
            NaclacError::ConstraintOwner => "Owner mismatch",
            NaclacError::ConstraintRentExempt => "Account is not rent exempt",
            NaclacError::ConstraintSeeds => "PDA derivation failed (Seeds mismatch)",
            NaclacError::ConstraintExecutable => "Account is not executable",
            NaclacError::ConstraintAccountIsNone => {
                "Account is None (Expected initialized account)"
            }
            NaclacError::ProgramIdMismatch => {
                "Program ID mismatch (Missing required program in accounts array)"
            }
            NaclacError::AccountDataTooSmall => {
                "Account data too small (Missing discriminator or insufficient size)"
            }
            NaclacError::AccountBorrowFailed => "Account borrow failed (Already borrowed)",
            NaclacError::InvalidInstructionData => "Invalid instruction data",
            NaclacError::InsufficientFunds => "Insufficient funds for transaction",
            NaclacError::AccountAlreadyInitialized => "Account already initialized",
            NaclacError::AccountNotInitialized => "Account not initialized",
            NaclacError::NotEnoughAccountKeys => "Not enough account keys provided",
            NaclacError::MaxSeedLengthExceeded => "Max seed length exceeded",
            NaclacError::UnsupportedSysvar => "Unsupported sysvar",
            NaclacError::InvalidRealloc => "Invalid reallocation",
            NaclacError::ArithmeticOverflow => "Arithmetic overflow",
            NaclacError::Unauthorized => "Unauthorized access (Authority mismatch)",
            NaclacError::InvalidAccountDiscriminator => {
                "Account type mismatch (Invalid discriminator)"
            }
            NaclacError::DeserializationFailed => "Account deserialization failed",
            NaclacError::SerializationFailed => "Account serialization failed",
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
            NaclacError::TooManyExtraAccounts => {
                "Too many extra accounts appended to a CPI (exceeds its fixed maximum)"
            }
            NaclacError::TooManyCpiAccounts => "Too many accounts in one CPI call",
            NaclacError::TooManyAccountEntries => {
                "Instruction carries more than 64 account entries"
            }
            NaclacError::CapacityExceeded => "Fixed-capacity vector or string is full",
            NaclacError::InvalidUtf8 => "Fixed-capacity string holds invalid UTF-8",
        }
    }
}

/// One past the highest framework error code: account index 255, the largest
/// index a transaction's account list can address.
const FRAMEWORK_ERROR_END: u32 = FRAMEWORK_ERROR_BASE + 256 * 100;

pub fn decode_custom_error(code: u32) -> Option<(NaclacError, usize)> {
    if !(FRAMEWORK_ERROR_BASE..FRAMEWORK_ERROR_END).contains(&code) {
        return None;
    }
    let offset_code = code - FRAMEWORK_ERROR_BASE;
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
        29 => NaclacError::TooManyExtraAccounts,
        30 => NaclacError::TooManyCpiAccounts,
        31 => NaclacError::TooManyAccountEntries,
        32 => NaclacError::CapacityExceeded,
        33 => NaclacError::InvalidUtf8,
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
    /// subtraction (`code - FRAMEWORK_ERROR_BASE`) is guarded by the range check
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
            let reconstructed = 10_000 + (index as u32) * 100 + (error as u32);
            assert_eq!(
                reconstructed, code,
                "decode must be a true inverse of the encoder"
            );
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

#[cfg(test)]
mod table_parity_tests {
    use super::*;

    const TYPESCRIPT_TABLE: &str = include_str!("../../client/src/utils/errors.ts");

    /// `(code, name, message)` of every entry of `NACLAC_FRAMEWORK_ERRORS` in the
    /// TypeScript client's `errors.ts`.
    fn typescript_entries() -> Vec<(u32, String, String)> {
        TYPESCRIPT_TABLE
            .lines()
            .filter_map(|line| {
                let entry = line.trim().strip_prefix("{ code: ")?;
                let (code, rest) = entry.split_once(", name: \"")?;
                let (name, rest) = rest.split_once("\", message: \"")?;
                let (message, _) = rest.split_once("\" }")?;
                Some((code.parse().ok()?, name.to_string(), message.to_string()))
            })
            .collect()
    }

    #[test]
    fn the_typescript_table_matches_this_enum_entry_for_entry() {
        let rust: Vec<(u32, String, String)> = NaclacError::ALL
            .iter()
            .map(|e| (*e as u32, format!("{e:?}"), e.description().to_string()))
            .collect();
        assert_eq!(typescript_entries(), rust);
    }

    macro_rules! assert_matches_core {
        ($($variant:ident),* $(,)?) => {
            $(assert_eq!(
                NaclacError::$variant as u32,
                naclac_core::error::NaclacError::$variant as u32,
                stringify!($variant)
            );)*
            let listed = [$(NaclacError::$variant),*];
            assert_eq!(listed, NaclacError::ALL);
        };
    }

    #[test]
    fn every_variant_has_the_same_code_as_naclac_core() {
        assert_matches_core!(
            ConstraintMut,
            ConstraintSigner,
            ConstraintAddress,
            ConstraintOwner,
            ConstraintRentExempt,
            ConstraintSeeds,
            ConstraintExecutable,
            ConstraintAccountIsNone,
            ProgramIdMismatch,
            AccountDataTooSmall,
            AccountBorrowFailed,
            InvalidInstructionData,
            InsufficientFunds,
            AccountAlreadyInitialized,
            AccountNotInitialized,
            NotEnoughAccountKeys,
            MaxSeedLengthExceeded,
            UnsupportedSysvar,
            InvalidRealloc,
            ArithmeticOverflow,
            Unauthorized,
            InvalidAccountDiscriminator,
            DeserializationFailed,
            SerializationFailed,
            ConstraintDuplicateMutableAccount,
            ConstraintClose,
            TooManyCpiSigners,
            TooManyCpiSeeds,
            TooManyExtraAccounts,
            TooManyCpiAccounts,
            TooManyAccountEntries,
            CapacityExceeded,
            InvalidUtf8,
        );
    }

    #[test]
    fn decoding_inverts_the_code_of_every_variant() {
        for error in NaclacError::ALL {
            let code = FRAMEWORK_ERROR_BASE + 7 * 100 + error as u32;
            assert_eq!(decode_custom_error(code), Some((error, 7)));
        }
    }
}
