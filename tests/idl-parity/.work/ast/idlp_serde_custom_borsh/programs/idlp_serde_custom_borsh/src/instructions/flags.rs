//! Constraint keywords that set a flag or pin an address/owner, in every accepted form.

use crate::constants::TREASURY;
use naclac_lang::prelude::*;

#[derive(Accounts)]
pub struct ConstraintFlags {
    /// SAFETY: only its signature is checked.
    #[account(signer)]
    pub signed_raw: AccountInfo,
    /// SAFETY: only mutability is requested.
    #[account(mut)]
    pub writable_raw: AccountInfo,
    /// SAFETY: only the executable flag is checked.
    #[account(executable)]
    pub program_raw: AccountInfo,
    /// SAFETY: only rent exemption is checked.
    #[account(rent_exempt)]
    pub rent_raw: AccountInfo,
    /// SAFETY: only the owner is checked.
    #[account(owner = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub owned_by_system: AccountInfo,
    /// SAFETY: only the owner is checked against a sibling account.
    #[account(owner = expected_owner)]
    pub owned_by_field: AccountInfo,
    /// SAFETY: used only as an owner reference.
    pub expected_owner: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = crate::ID)]
    pub is_self: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]
    pub is_system: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = naclac_lang::prelude::TOKEN_PROGRAM_ID)]
    pub is_token: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = naclac_lang::prelude::TOKEN_2022_PROGRAM_ID)]
    pub is_token_2022: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = naclac_lang::prelude::ASSOCIATED_TOKEN_PROGRAM_ID)]
    pub is_ata_program: AccountInfo,
    /// SAFETY: only its address is checked.
    #[account(address = naclac_lang::prelude::RENT_SYSVAR_ID)]
    pub is_rent: AccountInfo,
    /// SAFETY: only its address is checked against a crate constant.
    #[account(address = TREASURY)]
    pub is_treasury: AccountInfo,
    /// SAFETY: only its address is checked against a crate constant by path.
    #[account(address = crate::constants::OTHER_PROGRAM)]
    pub is_other_program: AccountInfo,
    /// SAFETY: only its address is checked against the metadata program.
    #[account(address = naclac_lang::metadata::ID)]
    pub is_metadata_program: AccountInfo,
    /// SAFETY: only its address is checked against a sibling account.
    #[account(address = other_account)]
    pub relational: AccountInfo,
    /// SAFETY: used only as an address reference.
    pub other_account: AccountInfo,
    /// SAFETY: aliased on purpose, only mutability is requested.
    #[account(mut, unsafe(alias))]
    pub aliased: AccountInfo,
}

pub fn constraint_flags(_ctx: Context<ConstraintFlags>) -> Result {
    Ok(())
}
