use naclac_lang::prelude::*;

// A genuine `NaclacError` (`ConstraintAddress`) that actually reaches the
// chain — unlike a missing `Signer` signature, which Solana's own
// client-side transaction-signing rules reject *before* the transaction is
// ever sent (no signature for a declared signer means the transaction
// can't be constructed at all, so the on-chain `ConstraintSigner` check
// never runs). `address = <const>` has no such client-side gate: a wrong
// address is a perfectly valid, signable transaction that fails on-chain.
#[derive(Accounts)]
pub struct CheckFixedAddress {
    /// SAFETY: only this account's own address is checked against the
    /// constant below — its data is never read or deserialized.
    #[account(address = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub target: AccountInfo,
}

#[instruction]
pub fn check_fixed_address(_ctx: Context<CheckFixedAddress>) -> Result {
    Ok(())
}
