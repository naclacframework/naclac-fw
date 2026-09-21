use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// Real `approve`/`approve_signed` CPI (`naclac-token/src/token.rs`), signed
// by the `mint_authority` PDA (the vault's real owner). `delegate` is a bare
// `AccountInfo` — only its address is used, mirroring
// `set_mint_authority.rs`'s `new_authority` reasoning.
#[derive(Accounts)]
pub struct ApproveVaultDelegate {
    #[account(mut)]
    pub vault: Account<TokenAccount>,

    /// SAFETY: only used as an address — becomes the approved delegate via
    /// `approve_signed` below; its data is never read or deserialized.
    pub delegate: AccountInfo,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token>,
}

pub fn approve_vault_delegate(ctx: Context<ApproveVaultDelegate>, amount: u64) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.approve_signed(
        naclac_lang::prelude::ApproveAccounts {
            to: &mut ctx.accounts.vault,
            delegate: &ctx.accounts.delegate,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        signer,
    )?;

    Ok(())
}
