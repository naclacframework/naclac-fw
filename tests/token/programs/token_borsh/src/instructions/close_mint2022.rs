use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof that `MintCloseAuthority` actually permits closing
/// a Token-2022 mint via the ordinary `close_account`/`close_account_signed`
/// CPI (`naclac-token/src/token.rs`) — no dedicated "close mint" action
/// exists in the real protocol, this extension only unlocks the standard
/// `CloseAccount` instruction for a supply-zero mint. Signed by the
/// `mint_authority` PDA — the same authority `create_mint2022_with_mint_close_authority.rs`
/// registers as the mint's `close_authority`.
#[derive(Accounts)]
pub struct CloseMint2022 {
    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,

    /// SAFETY: only used as the lamport-destination address for
    /// `close_account_signed` below; its data is never read or deserialized.
    #[account(mut)]
    pub destination: AccountInfo,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn close_mint2022(ctx: Context<CloseMint2022>) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    ctx.accounts.token_program.close_account_signed(
        naclac_lang::prelude::CloseAccountAccounts {
            account: &mut ctx.accounts.mint,
            destination: &mut ctx.accounts.destination,
            authority: &ctx.accounts.mint_authority,
        },
        signer,
    )?;

    Ok(())
}
