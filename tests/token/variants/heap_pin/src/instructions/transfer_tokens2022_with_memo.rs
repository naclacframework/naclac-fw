use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof that Token-2022's `MemoTransfer` enforcement
/// (`check_previous_sibling_instruction_is_memo`, verified against the real
/// `spl-token-2022-11.0.0` processor) actually accepts a transfer preceded
/// by a real memo CPI at the same call depth. The memo instruction is
/// hand-built rather than going through `pinocchio_memo::instructions::Memo`
/// — that helper is `#[inline(always)]` and unconditionally stack-allocates
/// a 64-slot account array (`MAX_STATIC_CPI_ACCOUNTS`), which is more than
/// this function's real 4096-byte SBF stack frame can afford alongside the
/// `transfer_checked_signed` CPI below. The real memo call needs zero
/// accounts here (no signers), so this builds the `InstructionView`
/// directly and calls it via `cpi::invoke_pinocchio` — an ordinary function
/// call, not inlined, keeping its own internal array off this function's
/// frame. `memo_program` below exists purely so the real memo program is
/// part of this instruction's own account list for the runtime to resolve
/// the sibling CPI against — it's never referenced by the memo CPI itself,
/// which targets `pinocchio_memo::ID` directly.
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

    /// SAFETY: never read or deserialized — present only so the real
    /// spl-memo program (v1 or v3) is part of this instruction's own
    /// account list. Not SPL-token-shaped, no naclac `Discriminator`
    /// applies.
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

    let memo_ix = instruction::InstructionView {
        program_id: &::pinocchio_memo::ID,
        accounts: &[],
        data: b"ok",
    };
    cpi::invoke_pinocchio(&memo_ix, &[])?;

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
