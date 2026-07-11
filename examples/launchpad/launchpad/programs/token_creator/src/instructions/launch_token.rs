use naclac_lang::prelude::*;
use amm_client::{Amm, InitializeCpi};
use crate::components::launch_record::LaunchRecord;
use crate::constants::SEED_MINT;

#[instruction_args]
pub struct LaunchTokenArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub launch_record_bump: u8,
    pub pool_bump: u8,
    pub decimals: u8,
    pub amount_token_pool: u64,
    pub amount_token_launcher: u64,
    pub amount_quote: u64,
}

#[derive(Accounts)]
#[instruction(args: LaunchTokenArgs)]
pub struct LaunchToken {
    #[account(mut)]
    pub payer: Signer,

    pub quote_mint: Account<Mint>,

    #[account(
        mut,
        seeds = [SEED_MINT, &args.id.to_le_bytes()],
        bump = args.mint_bump
    )]
    pub mint: AccountInfo,

    #[account(
        init,
        payer = payer,
        seeds = [b"launch", payer.address().as_ref(), &args.id.to_le_bytes()],
        bump = args.launch_record_bump
    )]
    pub launch_record: Account<LaunchRecord>,

    #[account(mut)]
    pub launcher_token_a: Account<TokenAccount>,

    #[account(mut)]
    pub launcher_token_b: Account<TokenAccount>,

    #[account(mut)]
    pub launcher_lp: Account<TokenAccount>,

    #[account(mut)]
    pub payer_token_b: Account<TokenAccount>,

    #[account(mut)]
    pub pool_state: AccountInfo,

    #[account(mut)]
    pub pool_vault_a: AccountInfo,

    #[account(mut)]
    pub pool_vault_b: AccountInfo,

    #[account(mut)]
    pub pool_lp_mint: AccountInfo,

    pub amm_program: Program<Amm>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[instruction]
pub fn launch_token(
    ctx: Context<LaunchToken>,
    args: LaunchTokenArgs,
) -> Result {
    let id_bytes = args.id.to_le_bytes();

    // 3. Transfer Token B (quote) from payer to launcher_token_b (vault owned by launch_record)
    ctx.accounts.token_program.transfer(
        naclac_lang::prelude::TransferAccounts {
            from: &mut ctx.accounts.payer_token_b,
            to: &mut ctx.accounts.launcher_token_b,
            authority: &ctx.accounts.payer,
        },
        args.amount_quote,
    )?;

    // 4. Mint total supply of Token A to launcher_token_a
    let payer_address = ctx.accounts.payer.address();
    let record_seeds: &[&[u8]] = &[
        b"launch",
        payer_address.as_ref(),
        &id_bytes,
        &[args.launch_record_bump],
    ];
    let record_signer: &[&[&[u8]]] = &[record_seeds];

    let amount_token_total = args.amount_token_pool + args.amount_token_launcher;

    ctx.accounts.token_program.mint_to_signed(
        naclac_lang::prelude::MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.launcher_token_a,
            authority: &ctx.accounts.launch_record,
        },
        amount_token_total,
        record_signer,
    )?;

    // 5. CPI to AMM program to initialize the pool
    ctx.accounts.amm_program.initialize_signed(
        amm_client::instructions::InitializeCpiAccounts {
            payer: ctx.accounts.payer.to_cpi_handle_mut(),
            token_a_mint: ctx.accounts.mint.to_cpi_handle(),
            token_b_mint: ctx.accounts.quote_mint.to_cpi_handle(),
            pool_state: ctx.accounts.pool_state.to_cpi_handle_mut(),
            vault_a: ctx.accounts.pool_vault_a.to_cpi_handle_mut(),
            vault_b: ctx.accounts.pool_vault_b.to_cpi_handle_mut(),
            lp_mint: ctx.accounts.pool_lp_mint.to_cpi_handle_mut(),
            depositor_token_a: ctx.accounts.launcher_token_a.to_cpi_handle_mut(),
            depositor_token_b: ctx.accounts.launcher_token_b.to_cpi_handle_mut(),
            depositor_lp: ctx.accounts.launcher_lp.to_cpi_handle_mut(),
            depositor_authority: ctx.accounts.launch_record.to_cpi_handle(),
            token_program: ctx.accounts.token_program.to_cpi_handle(),
            system_program: ctx.accounts.system_program.to_cpi_handle(),
        },
        args.id,
        args.pool_bump,
        args.amount_token_pool,
        args.amount_quote,
        record_signer,
    )?;

    // 6. Record launch in launch_record state using launch system
    crate::systems::process_launch_record(
        &mut ctx.accounts.launch_record,
        ctx.accounts.payer.address(),
        ctx.accounts.mint.address(),
        args.amount_token_pool,
        args.amount_quote,
    )?;

    // 7. Emit launch event
    emit!(crate::events::TokenLaunched {
        id: args.id,
        mint: ctx.accounts.mint.address(),
        amount_token: args.amount_token_pool,
        amount_quote: args.amount_quote,
    });

    Ok(())
}
