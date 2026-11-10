use naclac_lang::prelude::*;

// `token::token_program`: validates an existing (non-`init`) account's owning
// program against an expected `Program<Token>` field expression
// (`token::token_program = token_program`). Checked when the `vault` field
// loads: a mismatch is `NaclacError::ProgramIdMismatch`, a different error
// variant from the `ConstraintTokenMint`/`ConstraintTokenAuthority` that
// `token::mint`/`token::authority` return.
// Uses a bare `AccountInfo` for
// `vault` (not `Account<TokenAccount>`) since only the owning-program
// metadata is inspected here, never the account's data.
#[derive(Accounts)]
pub struct CheckVaultProgram {
    pub token_program: Program<Token>,

    /// SAFETY: only the account's owning-program metadata is inspected via
    /// the `token::token_program` constraint below; its data is never read or
    /// deserialized.
    #[account(token::token_program = token_program)]
    pub vault: AccountInfo,
}

pub fn check_vault_program(_ctx: Context<CheckVaultProgram>) -> Result {
    Ok(())
}
