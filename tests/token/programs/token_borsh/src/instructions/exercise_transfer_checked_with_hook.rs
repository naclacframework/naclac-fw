use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `transfer_checked_with_hook`
/// (`naclac-token/src/extensions/transfer_hook.rs`) — not just that
/// `initialize_transfer_hook`/`transfer_hook_update` are readable, but that
/// a real transfer against a hook-gated mint actually resolves the hook's
/// extra accounts and CPIs into the real hook program. `hook_program` plus
/// the hook's `ExtraAccountMetaList` PDA (passed via `remaining_accounts`,
/// the documented mechanism in `naclac-client/src/builder.rs` for exactly
/// this "token-2022 extensions" case) are combined into the `extra_accounts`
/// list `transfer_checked_with_hook_signed` requires.
#[derive(Accounts)]
pub struct ExerciseTransferCheckedWithHook {
    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub source: InterfaceAccount<TokenAccount>,

    #[account(mut)]
    pub destination: InterfaceAccount<TokenAccount>,

    /// SAFETY: only used as an address (the hook program to CPI into) and
    /// passed through to `transfer_checked_with_hook_signed`, which itself
    /// resolves and validates the hook's required accounts against real
    /// Token-2022/`spl-transfer-hook-interface` rules; its data is never
    /// read or deserialized here.
    pub hook_program: AccountInfo,

    pub token_program: Program<Token2022>,
}

#[instruction]
pub fn exercise_transfer_checked_with_hook(
    ctx: Context<ExerciseTransferCheckedWithHook>,
    amount: u64,
    decimals: u8,
) -> Result {
    let bump = ctx.accounts.mint_authority.bump;
    let signer_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let signer: &[&[&[u8]]] = &[signer_seeds];

    let mut extra_accounts: Vec<CpiHandle<'_>> = vec![ctx.accounts.hook_program.to_cpi_handle()];
    extra_accounts.extend(ctx.remaining_accounts.iter().map(|info| info.to_cpi_handle()));

    naclac_lang::prelude::transfer_checked_with_hook_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.source.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.destination.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        ctx.accounts.hook_program.to_cpi_handle(),
        &extra_accounts,
        amount,
        decimals,
        signer,
    )?;

    Ok(())
}
