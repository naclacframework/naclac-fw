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
    ConstraintDuplicateMutableAccount = 26,
    ConstraintClose = 27,
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
            NaclacError::ConstraintHasOne => "Account relational constraint (has_one) failed",
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
        9 => NaclacError::ConstraintHasOne,
        10 => NaclacError::ProgramIdMismatch,
        11 => NaclacError::AccountDataTooSmall,
        12 => NaclacError::AccountBorrowFailed,
        13 => NaclacError::InvalidInstructionData,
        14 => NaclacError::InsufficientFunds,
        15 => NaclacError::AccountAlreadyInitialized,
        16 => NaclacError::AccountNotInitialized,
        17 => NaclacError::NotEnoughAccountKeys,
        18 => NaclacError::MaxSeedLengthExceeded,
        19 => NaclacError::UnsupportedSysvar,
        20 => NaclacError::InvalidRealloc,
        21 => NaclacError::ArithmeticOverflow,
        22 => NaclacError::Unauthorized,
        23 => NaclacError::InvalidAccountDiscriminator,
        24 => NaclacError::DeserializationFailed,
        25 => NaclacError::SerializationFailed,
        26 => NaclacError::ConstraintDuplicateMutableAccount,
        27 => NaclacError::ConstraintClose,
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

pub fn translate_error_code(code: u32, account_names: Option<&[&str]>) -> String {
    if let Some((error, index)) = decode_custom_error(code) {
        let account_desc = if let Some(names) = account_names {
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
