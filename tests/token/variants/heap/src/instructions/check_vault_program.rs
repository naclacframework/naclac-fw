use naclac_lang::prelude::*;

// `token::program`: validates an existing (non-`init`) account's owning
// program against an expected `Program<Token>` field expression
// (`token::program = token_program`). Checked in
// `generate_relational_checks` (`security.rs`), which runs post
// account-load, not as part of the per-account metadata checks:
// `Owner::program_owner(&vault) == ToAddress::address(&token_program)`.
// Mismatch is `NaclacError::ProgramIdMismatch` — a genuinely different
// error variant from `token::mint`'s `ConstraintAccountIsNone` and
// `token::authority`'s `ConstraintAddress`, confirmed by reading the
// codegen directly rather than assumed. Uses a bare `AccountInfo` for
// `vault` (not `Account<TokenAccount>`) since only the owning-program
// metadata is inspected here, never the account's data.
#[derive(Accounts)]
pub struct CheckVaultProgram {
    pub token_program: Program<Token>,

    /// SAFETY: only the account's owning-program metadata is inspected via
    /// the `token::program` constraint below; its data is never read or
    /// deserialized.
    #[account(token::program = token_program)]
    pub vault: AccountInfo,
}

pub fn check_vault_program(_ctx: Context<CheckVaultProgram>) -> Result {
    Ok(())
}
