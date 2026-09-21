use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof that Token-2022's `MemoTransfer` enforcement
/// (`check_previous_sibling_instruction_is_memo`, verified against the real
/// `spl-token-2022-11.0.0` processor) actually accepts a transfer preceded
/// by a real memo CPI at the same call depth. Built via the real
/// `spl_memo_interface::instruction::build_memo` constructor (the same
/// approach `naclac-token`'s own `token_metadata.rs`/`token_group.rs` use
/// for their interface-crate CPIs), targeting whichever address the caller
/// passes as `memo_program` — this is test-only glue, not a naclac-token
/// production helper, so it doesn't independently verify that address is
/// really the v1/v3 memo program the way `validate_token_2022_program` does
/// for Token-2022 itself.
#[derive(Accounts)]
pub struct TransferTokens2022WithMemo {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub from: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub to: InterfaceAccount<TokenAccount>,

    pub token_program: Program<Token2022>,

    /// SAFETY: never read or deserialized as SPL-token-shaped data — only
    /// its address is used, to build and invoke the real spl-memo
    /// instruction below.
    pub memo_program: AccountInfo,
}

pub fn transfer_tokens2022_with_memo(
    ctx: Context<TransferTokens2022WithMemo>,
    amount: u64,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];
    let decimals = ctx.accounts.mint.decimals();

    let memo_program_address = ctx.accounts.memo_program.address();
    let memo_ix = spl_memo_interface::instruction::build_memo(&memo_program_address, b"ok", &[]);
    let memo_accounts = [ctx.accounts.memo_program.to_cpi_handle()];
    naclac_lang::prelude::cpi::invoke_signed(&memo_ix, &memo_accounts, &[])?;

    ctx.accounts.token_program.transfer_checked_signed(
        naclac_lang::prelude::TransferCheckedAccounts {
            from: &mut ctx.accounts.from,
            mint: &ctx.accounts.mint,
            to: &mut ctx.accounts.to,
            authority: &ctx.accounts.mint_authority,
        },
        amount,
        decimals,
        signer,
    )?;

    Ok(())
}
