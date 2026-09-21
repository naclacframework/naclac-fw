use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

// `set_authority`/`set_authority_signed` (`naclac-token/src/token.rs`), both
// authority kinds this test case exercises: `authority_type == 0` maps to
// `AuthorityType::MintTokens`, anything else to `AuthorityType::FreezeAccount`
// — a single instruction covers both variants rather than duplicating the
// whole shape twice. `new_authority` is passed as a bare `AccountInfo`
// account (its key only), not a raw `Address`/`Pubkey` instruction argument —
// `naclac-macros/src/accounts.rs`'s zero-copy-mode compile check explicitly
// bans a raw `Pubkey`/`Address` as an `#[instruction(...)]` argument, so this
// mirrors how every other cross-account key in this test case
// (`check_mint_freeze_authority`'s `freeze_authority`, `create_ata`'s
// `owner`) is passed.
#[derive(Accounts)]
pub struct SetMintAuthority {
    #[account(mut)]
    pub mint: Account<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    /// SAFETY: only used as an address — its own key becomes the mint's new
    /// authority via `set_authority_signed` below; its data is never read or
    /// deserialized.
    pub new_authority: AccountInfo,

    pub token_program: Program<Token>,
}

pub fn set_mint_authority(ctx: Context<SetMintAuthority>, authority_type: u8) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let auth_type = if authority_type == 0 {
        AuthorityType::MintTokens
    } else {
        AuthorityType::FreezeAccount
    };
    let new_authority_address = ctx.accounts.new_authority.address();

    ctx.accounts.token_program.set_authority_signed(
        naclac_lang::prelude::SetAuthorityAccounts {
            account_or_mint: &mut ctx.accounts.mint,
            current_authority: &ctx.accounts.mint_authority,
        },
        auth_type,
        Some(&new_authority_address),
        signer,
    )?;

    Ok(())
}
