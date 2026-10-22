use naclac_lang::prelude::*;

// `mint::freeze_authority` on an *existing* (non-`init`) mint —
// `security.rs`'s existing-account mint-layout check (only reachable when
// `field.init_config.is_none()`), reading the raw SPL `Mint` layout
// directly: bytes `[46..50]` are the freeze-authority `COption`
// discriminant (`[1, 0, 0, 0]` = Some), bytes `[50..82]` are the freeze
// authority pubkey if present. No freeze authority set at all ->
// `NaclacError::Unauthorized`; a freeze authority set but not matching the
// expected one -> `NaclacError::ConstraintAddress` — deliberately distinct
// error variants, confirmed by reading the codegen directly rather than
// assumed (mirrors the `Unauthorized`/`ConstraintAddress` split already
// proven for `mint::authority`).
#[derive(Accounts)]
pub struct CheckMintFreezeAuthority {
    /// SAFETY: only used as an address to compare the mint's freeze
    /// authority against, via the `mint::freeze_authority` constraint below;
    /// its data is never read or deserialized. Declared before `mint`
    /// because field constraints reference sibling fields by local variable
    /// name, and fields load sequentially in declaration order (same
    /// convention as `check_vault_constraints.rs`'s `mint`/`mint_authority`
    /// preceding `vault`).
    pub freeze_authority: AccountInfo,

    #[account(mint::freeze_authority = freeze_authority)]
    pub mint: Account<Mint>,
}

pub fn check_mint_freeze_authority(_ctx: Context<CheckMintFreezeAuthority>) -> Result {
    Ok(())
}
