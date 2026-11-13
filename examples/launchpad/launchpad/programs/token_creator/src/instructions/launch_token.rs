//! Single-transaction token launch: creates the mint, mints its supply to the
//! payer, seeds an AMM pool against a quote mint and locks the pool's first
//! liquidity shares under `launch_record`, which this program never moves.

use {
    crate::{
        constants::{SEED_LAUNCH, SEED_MINT},
        errors::LaunchpadError,
        events::TokenLaunched,
        state::launch_record::LaunchRecord,
        systems::process_launch_record,
    },
    amm_client::{Amm, InitializeCpi, InitializeCpiAccounts},
    naclac_lang::prelude::*,
};

#[instruction_args]
pub struct LaunchTokenArgs {
    pub id: u64,
    pub decimals: u8,
    pub amount_token_pool: u64,
    pub amount_token_launcher: u64,
    pub amount_quote: u64,
}

/// `pool_vault_a` and `pool_vault_b` follow the AMM's own order: `pool_vault_a`
/// holds whichever of `mint` and `quote_mint` has the lower address.
#[derive(Accounts)]
#[instruction(args: LaunchTokenArgs)]
pub struct LaunchToken {
    #[account(mut)]
    pub payer: Signer,

    pub quote_mint: InterfaceAccount<Mint>,

    #[account(
        init,
        payer = payer,
        seeds = [SEED_LAUNCH, payer.address().as_ref(), &args.id.to_le_bytes()],
        bump
    )]
    pub launch_record: Account<LaunchRecord>,

    /// SAFETY: `init` + `mint::*` below create and initialise this account
    /// through the Token program at an address only this program can sign for;
    /// it is a raw SPL `Mint` with no naclac discriminator to check.
    #[account(
        init,
        payer = payer,
        seeds = [SEED_MINT, payer.address().as_ref(), &args.id.to_le_bytes()],
        bump,
        mint::decimals = args.decimals,
        mint::authority = launch_record,
        mint::token_program = token_program,
    )]
    pub mint: AccountInfo,

    /// SAFETY: `init` + `associated_token::*` below have the Associated Token
    /// program create and address-check this account; it is a raw SPL token
    /// account with no naclac discriminator to check. The mint is created in
    /// this same instruction, so nobody can create this account first.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program,
    )]
    pub payer_token_account: AccountInfo,

    /// SAFETY: the AMM receives this as the depositor token account of
    /// `quote_mint` and checks its mint and its owner.
    #[account(mut)]
    pub payer_quote_account: AccountInfo,

    /// SAFETY: created and address-checked by the AMM `initialize` call that
    /// receives it.
    #[account(mut)]
    pub pool_state: AccountInfo,

    /// SAFETY: same as `pool_state`.
    #[account(mut)]
    pub pool_vault_a: AccountInfo,

    /// SAFETY: same as `pool_state`.
    #[account(mut)]
    pub pool_vault_b: AccountInfo,

    /// SAFETY: same as `pool_state`.
    #[account(mut)]
    pub pool_lp_mint: AccountInfo,

    /// SAFETY: the payer's LP associated token account, created and
    /// address-checked by the AMM `initialize` call that receives it; its owner
    /// is then reassigned to `launch_record`.
    #[account(mut)]
    pub payer_lp: AccountInfo,

    pub token_program: Program<Token>,

    /// SAFETY: repeats `token_program` when the quote mint is classic Token.
    #[account(dup)]
    pub quote_token_program: Interface<TokenInterface>,

    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
    pub amm_program: Program<Amm>,
}

/// Whether `mint` sorts below `quote_mint`, which makes it the AMM's token A.
/// The AMM requires its token A address to sort below its token B address.
fn mint_sorts_first(mint: &Address, quote_mint: &Address) -> bool {
    let mint: &[u8] = mint.as_ref();
    let quote_mint: &[u8] = quote_mint.as_ref();
    mint < quote_mint
}

pub fn launch_token(ctx: Context<LaunchToken>, args: LaunchTokenArgs) -> Result {
    let id = args.id;
    let amount_token_pool = args.amount_token_pool;
    let amount_token_launcher = args.amount_token_launcher;
    let amount_quote = args.amount_quote;

    let amount_token_total = amount_token_pool
        .checked_add(amount_token_launcher)
        .ok_or(LaunchpadError::Overflow)?;

    let payer_address = ctx.accounts.payer.address();
    let id_bytes = id.to_le_bytes();
    let launch_record_bump = [ctx.bumps.launch_record];
    let record_seeds = SignerSeeds::new([
        SEED_LAUNCH,
        payer_address.as_ref(),
        &id_bytes,
        &launch_record_bump,
    ]);
    let record_signer = &[record_seeds.signer()];

    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.payer_token_account,
            authority: &ctx.accounts.launch_record,
        },
        amount_token_total,
        record_signer,
    )?;

    ctx.accounts.token_program.set_authority_signed(
        SetAuthorityAccounts {
            account_or_mint: &mut ctx.accounts.mint,
            current_authority: &ctx.accounts.launch_record,
        },
        AuthorityType::MintTokens,
        None,
        record_signer,
    )?;

    let new_token_side = (
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.payer_token_account.to_cpi_handle_mut(),
        ctx.accounts.token_program.to_cpi_handle(),
        amount_token_pool,
    );
    let quote_side = (
        ctx.accounts.quote_mint.to_cpi_handle(),
        ctx.accounts.payer_quote_account.to_cpi_handle_mut(),
        ctx.accounts.quote_token_program.to_cpi_handle(),
        amount_quote,
    );
    let (side_a, side_b) = if mint_sorts_first(
        &ctx.accounts.mint.address(),
        &ctx.accounts.quote_mint.address(),
    ) {
        (new_token_side, quote_side)
    } else {
        (quote_side, new_token_side)
    };

    ctx.accounts.amm_program.initialize(
        InitializeCpiAccounts {
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            token_a_mint: side_a.0,
            token_b_mint: side_b.0,
            pool_state: ctx.accounts.pool_state.to_cpi_handle_mut(),
            lp_mint: ctx.accounts.pool_lp_mint.to_cpi_handle_mut(),
            vault_a: ctx.accounts.pool_vault_a.to_cpi_handle_mut(),
            vault_b: ctx.accounts.pool_vault_b.to_cpi_handle_mut(),
            depositor_token_a: side_a.1,
            depositor_token_b: side_b.1,
            depositor_lp: ctx.accounts.payer_lp.to_cpi_handle_mut(),
            token_program_a: side_a.2,
            token_program_b: side_b.2,
            lp_token_program: ctx.accounts.token_program.to_cpi_handle(),
            associated_token_program: ctx.accounts.associated_token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        id,
        side_a.3,
        side_b.3,
    )?;

    ctx.accounts.token_program.set_authority(
        SetAuthorityAccounts {
            account_or_mint: &mut ctx.accounts.payer_lp,
            current_authority: &ctx.accounts.payer,
        },
        AuthorityType::AccountOwner,
        Some(&ctx.accounts.launch_record.address()),
    )?;

    process_launch_record(
        &mut ctx.accounts.launch_record,
        payer_address,
        ctx.accounts.mint.address(),
        amount_token_pool,
        amount_quote,
    )?;

    emit!(TokenLaunched {
        id,
        mint: ctx.accounts.mint.address(),
        amount_token: amount_token_pool,
        amount_quote,
    });

    Ok(())
}
