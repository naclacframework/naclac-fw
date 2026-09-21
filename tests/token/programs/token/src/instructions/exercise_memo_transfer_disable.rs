use naclac_lang::prelude::*;

/// Real end-to-end proof of `disable_required_memo_transfers`
/// (`naclac-token/src/extensions/memo_transfer.rs`) — disables the
/// extension on an already-`MemoTransfer`-enabled vault via CPI, signed by
/// the account's owner, then reads it back to confirm the on-chain flag
/// actually flipped, not just that Token-2022 accepted the instruction.
#[derive(Accounts)]
pub struct ExerciseMemoTransferDisable {
    #[account(mut)]
    pub vault: InterfaceAccount<TokenAccount>,

    pub owner: Signer,

    pub token_program: Program<Token2022>,
}

pub fn exercise_memo_transfer_disable(ctx: Context<ExerciseMemoTransferDisable>) -> Result {
    disable_required_memo_transfers(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.vault.to_cpi_handle_mut(),
        ctx.accounts.owner.to_cpi_handle(),
    )?;

    let ext: MemoTransfer = ctx.accounts.vault.get_extension()?;
    if ext.require_incoming_transfer_memos() {
        return Err(NaclacError::ConstraintAddress.into());
    }

    Ok(())
}
