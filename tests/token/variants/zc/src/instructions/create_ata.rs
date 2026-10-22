use naclac_lang::prelude::*;

// Real Associated Token Account creation via the declarative
// `associated_token::mint`/`associated_token::authority` constraint
// (`init_cpi.rs`) — mirrors `mint::decimals`/`mint::authority`'s existing
// declarative sugar exactly, rather than a manual CPI call in the
// instruction body. Previously this instruction called
// `AssociatedTokenCpi::create` by hand because this constraint didn't
// exist yet; now that it does, `create_ata`'s body is empty, same as
// `create_mint`'s. `associated_token` is a bare `AccountInfo` (not
// `Account<TokenAccount>`), since it doesn't exist on-chain yet — the CPI
// itself creates it, matching `create_mint.rs`'s `mint: AccountInfo`
// reasoning.
//
// Named `CreateAssociatedTokenAccount`, not `CreateAta`: `naclac_lang::prelude`
// already re-exports `associated_token::Create as CreateAta` (the raw CPI
// accounts struct) — an `Accounts` struct of the same name collides via the
// two glob imports (`naclac_lang::prelude::*` and `instructions::*`) `lib.rs`
// uses, producing an ambiguous-name error at the `Context<CreateAta>` call
// site.
#[derive(Accounts)]
pub struct CreateAssociatedTokenAccount {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: only used as the ATA's `authority` argument to the
    /// Associated Token Program CPI below; never read or deserialized.
    pub owner: AccountInfo,

    pub mint: Account<Mint>,

    /// SAFETY: this account doesn't exist yet — the Associated Token
    /// Program CPI (`associated_token::mint`/`associated_token::authority`
    /// below) creates it. There is no naclac `Discriminator` to check since
    /// this is a raw SPL `TokenAccount` layout created by a separate
    /// program.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub associated_token: AccountInfo,

    pub system_program: Program<System>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
}

pub fn create_ata(_ctx: Context<CreateAssociatedTokenAccount>) -> Result {
    Ok(())
}
