use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT;

/// Real end-to-end proof of `initialize_group_member_pointer`
/// (`naclac-token/src/extensions/group_member_pointer.rs`): allocates the
/// mint PDA by hand (same reason as `create_mint2022_with_transfer_fee.rs`
/// — `InitializeGroupMemberPointer` must run before `InitializeMint`).
#[instruction_args]
pub struct CreateMint2022WithGroupMemberPointerArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
    pub member_address: Address,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithGroupMemberPointerArgs)]
pub struct CreateMint2022WithGroupMemberPointer {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below (`create_account_signed` then two CPIs) — there is no
    /// naclac `Discriminator` to check since this is a raw SPL `Mint` +
    /// `GroupMemberPointer` extension layout, not a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 64-byte `GroupMemberPointer` value.
const MINT_WITH_GROUP_MEMBER_POINTER_SPACE: u64 = 82 + 83 + 1 + 4 + 64;

pub fn create_mint2022_with_group_member_pointer(
    ctx: Context<CreateMint2022WithGroupMemberPointer>,
    args: CreateMint2022WithGroupMemberPointerArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_GROUP_MEMBER_POINTER_SPACE as usize)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_GROUP_MEMBER_POINTER_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_group_member_pointer(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        Some(&args.member_address),
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        args.decimals,
        &mint_authority_address,
        None,
    )?;

    Ok(())
}
