use naclac_lang::prelude::*;

#[error_code]
pub enum PumpError {
    /// Signer does not match the authority this instruction requires --
    /// which `Global` field that is varies per instruction, not always
    /// `authority` itself.
    NotAuthorized,
    /// Remaining accounts count doesn't match what this instruction expects
    NotEnoughRemainingAccounts,
    /// A remaining account's address doesn't match the expected shareholder
    ShareholderAccountMismatch,
    /// Only native-SOL creator vaults are supported here
    UnsupportedQuoteMint,
    /// The passed quote mint doesn't match the bonding curve's quote mint
    QuoteMintMismatch,
    /// Quote mint is not in the whitelist
    QuoteMintNotWhitelisted,
    /// Slippage tolerance exceeded
    SlippageExceeded,
    /// Math overflow
    MathOverflow,
    /// Missing or malformed fee data
    MissingFeesReturnData,
    /// Not enough tokens available to buy
    NotEnoughTokensToBuy,
    /// Fee recipient account is not rent-exempt
    FeeRecipientNotRentExempt,
    /// Invalid fee recipient
    InvalidFeeRecipient,
    /// Invalid buyback fee recipient
    InvalidBuybackFeeRecipient,
    /// Bonding curve is not complete
    NotComplete,
    /// Migration is disabled
    MigrateDisabled,
    /// Invalid withdraw authority
    InvalidWithdrawAuthority,
    /// Unsupported quote mint for migration
    UnsupportedQuoteMintForMigrate,
    /// Pool migration fee too high
    PoolMigrationFeeTooHigh,
    /// Minimum tokens out must be greater than zero
    ZeroMinTokensOut,
    /// Buy amount cannot be zero
    BuyZeroAmount,
    /// Sell amount cannot be zero
    SellZeroAmount,
    /// Quote mint whitelist is full
    QuoteMintWhitelistFull,
    /// Quote mint is already whitelisted
    QuoteMintAlreadyWhitelisted,
    /// Quote mint cannot be added or removed via whitelist (default or native SOL mint)
    QuoteMintNotEligibleForWhitelist,
    /// Create v2: quote token program must be legacy SPL Token
    InvalidQuoteTokenProgram,
    /// create_v2 is currently disabled by the admin
    CreateV2Disabled,
    /// Cashback is not enabled
    CashbackNotEnabled,
    /// Mayhem mode is not enabled by the admin
    MayhemModeDisabled,
    /// buyback fee recipients require exactly 8 remaining accounts (or none)
    WrongBuybackFeeRecipientsCount,
    /// Bonding curve creator does not match sharing config
    BondingCurveAndSharingConfigCreatorMismatch,
    /// creator_vault has been migrated to sharing config, use
    /// distribute_creator_fees(_v2) instead
    UnableToDistributeCreatorVaultMigratedToSharingConfig,
    /// The recipient account is executable, so it cannot receive lamports;
    /// remove it from the team first
    UnableToDistributeCreatorFeesToExecutableRecipient,
}
