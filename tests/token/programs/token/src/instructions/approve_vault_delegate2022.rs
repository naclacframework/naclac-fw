use naclac_lang::prelude::*;

/// Real CPI-issued `Approve` (`naclac-token/src/token.rs`), signed by a real
/// account owner (not a PDA) — used specifically to prove `CpiGuard`
/// enforcement: real Token-2022 unconditionally rejects `Approve` via CPI
/// once `CpiGuard` is enabled on the target account, regardless of who's
/// calling or why.
#[derive(Accounts)]
pub struct ApproveVaultDelegate2022 {
    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    /// SAFETY: only used as an address — becomes the approved delegate via
    /// `approve` below; its data is never read or deserialized.
    pub delegate: AccountInfo,

    pub owner: Signer,

    pub token_program: Program<Token2022>,
}

pub fn approve_vault_delegate2022(ctx: Context<ApproveVaultDelegate2022>, amount: u64) -> Result {
    ctx.accounts.token_program.approve(
        naclac_lang::prelude::ApproveAccounts {
            to: &mut ctx.accounts.vault,
            delegate: &ctx.accounts.delegate,
            authority: &ctx.accounts.owner,
        },
        amount,
    )?;

    Ok(())
}
