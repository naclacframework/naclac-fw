use naclac_lang::prelude::*;

/// Real end-to-end proof of `enable_required_memo_transfers`
/// (`naclac-token/src/extensions/memo_transfer.rs`): allocates a plain
/// (non-PDA) token account by hand with extra room for the `MemoTransfer`
/// TLV entry, initializes it normally, then enables the extension — unlike
/// `ImmutableOwner`, `Enable`/`Disable` run *after* `initialize_account`
/// (the account must already have a real owner for the owner-signature
/// check), not before.
#[derive(Accounts)]
pub struct CreateTokenAccountWithMemoTransferRequired {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account` then three CPIs) — there is no naclac
    /// `Discriminator` to check since this is a raw SPL `TokenAccount` +
    /// `MemoTransfer` extension layout, not a naclac component.
    #[account(mut)]
    pub token_account: Signer,

    pub mint: InterfaceAccount<Mint>,

    pub owner: Signer,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 165 bytes + 1-byte `AccountType` marker + 4-byte TLV header + 1-byte
/// `MemoTransfer` value.
const TOKEN_ACCOUNT_WITH_MEMO_TRANSFER_SPACE: u64 = 165 + 1 + 4 + 1;

#[instruction]
pub fn create_token_account_with_memo_transfer_required(
    ctx: Context<CreateTokenAccountWithMemoTransferRequired>,
) -> Result {
    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(TOKEN_ACCOUNT_WITH_MEMO_TRANSFER_SPACE as usize)?;

    system_program::create_account(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        TOKEN_ACCOUNT_WITH_MEMO_TRANSFER_SPACE,
        &ctx.accounts.token_program.address(),
    )?;

    naclac_lang::prelude::initialize_account(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle(),
        ctx.accounts.owner.to_cpi_handle(),
    )?;

    naclac_lang::prelude::enable_required_memo_transfers(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.token_account.to_cpi_handle_mut(),
        ctx.accounts.owner.to_cpi_handle(),
    )?;

    Ok(())
}
