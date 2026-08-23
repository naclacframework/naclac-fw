use naclac_lang::prelude::*;

#[error_code]
pub enum FeesError {
    /// Only Pump and PumpSwap programs can call this instruction
    UnauthorizedProgram,
    /// Invalid admin
    InvalidAdmin,
    /// No fee tiers provided
    NoFeeTiers,
    /// Too many fee tiers (max 50)
    TooManyFeeTiers,
    /// The offset should be <= fee_config.fee_tiers.len()
    OffsetNotContinuous,
    /// Fee tiers must be sorted by market cap threshold (ascending)
    FeeTiersNotSorted,
    /// Fee total must not exceed 10_000bps
    InvalidFeeTotal,
    /// Invalid Sharing Config
    InvalidSharingConfig,
    /// Invalid Pool
    InvalidPool,
    /// Sharing config authority has been revoked - sharing config can only be updated once
    SharingConfigAdminRevoked,
    /// No shareholders provided
    NoShareholders,
    /// Too many shareholders (max 10)
    TooManyShareholders,
    /// Duplicate shareholder address
    DuplicateShareholder,
    /// Not enough remaining accounts
    NotEnoughRemainingAccounts,
    /// Invalid share total - must equal 10_000 basis points
    InvalidShareTotal,
    /// Share calculation overflow
    ShareCalculationOverflow,
    /// The given account is not authorized to execute this instruction.
    NotAuthorized,
    /// Shareholder cannot have zero share
    ZeroShareNotAllowed,
    /// Fee sharing config is not active
    SharingConfigNotActive,
    /// AMM accounts are required for graduated coins
    AmmAccountsRequiredForGraduatedCoin,
    /// Remaining account key doesn't match shareholder address
    ShareholderAccountMismatch,
    /// Feature is currently deactivated
    FeatureDeactivated,
    /// User ID exceeds maximum length
    UserIdTooLong,
    /// Instruction is deprecated
    DeprecatedInstruction,
    /// Reward split can only be updated once
    FeeSharesAlreadyUpdated,
    /// Math overflow
    MathOverflow,
    /// Invalid buyback index
    InvalidBuybackIndex,
    /// Claim rate limit exceeded
    ClaimRateLimitExceeded,
    /// Account is not a valid FeeConfig for this instruction
    InvalidFeeConfigAccount,
    /// Account type not supported
    AccountTypeNotSupported,
    /// Mint does not match quote mint
    InvalidMint,
    /// Unsupported quote mint
    UnsupportedQuoteMint,
}
