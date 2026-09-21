use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::SEED_MINT_AUTHORITY;

/// Real end-to-end proof of `initialize_token_group_member`
/// (`naclac-token/src/extensions/token_group.rs`): allocates a second,
/// member mint PDA, points a self-referential `GroupMemberPointer` at
/// itself, initializes the mint, then joins it to an already-initialized
/// `TokenGroup` on `group_mint` (created by
/// `create_mint2022_with_group_pointer_and_group`), signed by the group's
/// own PDA update authority.
#[instruction_args]
pub struct CreateMint2022WithGroupMemberPointerAndMemberArgs {
    pub member_seed: u64,
    pub member_mint_bump: u8,
    pub decimals: u8,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithGroupMemberPointerAndMemberArgs)]
pub struct CreateMint2022WithGroupMemberPointerAndMember {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below — raw SPL `Mint` + `GroupMemberPointer` +
    /// `TokenGroupMember` layout, not a naclac component.
    #[account(mut, seeds = [b"member_mint", &args.member_seed.to_le_bytes()], bump = args.member_mint_bump)]
    pub member_mint: AccountInfo,

    #[account(mut)]
    pub group_mint: InterfaceAccount<Mint>,

    #[account(seeds = [SEED_MINT_AUTHORITY], bump = mint_authority.bump)]
    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 + 83 padding + 1-byte `AccountType` + 4-byte `GroupMemberPointer`
/// TLV header + 64-byte value — the account's data length at
/// `create_account` time. `InitializeMember`'s own processor grows the
/// account itself via the same self-reallocating `alloc_and_serialize`
/// mechanism `InitializeGroup` uses (see
/// `create_mint2022_with_group_pointer_and_group.rs`'s own doc comment for
/// why pre-allocating `TokenGroupMember`'s space here would make
/// `InitializeMint2` reject the account as `InvalidAccountData`).
const MEMBER_MINT_SPACE: u64 = 82 + 83 + 1 + 4 + 64;

/// Lamports funded for `MEMBER_MINT_SPACE + 4 + 72` (`TokenGroupMember` TLV
/// header + `size_of::<TokenGroupMember>()`: 32 `mint` + 32 `group` + 8
/// `member_number`) rounded up generously, since `resize` inside
/// `InitializeMember` never adds lamports.
const MEMBER_MINT_FUNDED_SPACE: usize = 400;

pub fn create_mint2022_with_group_member_pointer_and_member(
    ctx: Context<CreateMint2022WithGroupMemberPointerAndMember>,
    args: CreateMint2022WithGroupMemberPointerAndMemberArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let member_mint_address = ctx.accounts.member_mint.address();
    let seed_bytes = args.member_seed.to_le_bytes();
    let member_seeds: &[&[u8]] = &[b"member_mint", &seed_bytes, &[args.member_mint_bump]];
    let member_signer: &[&[&[u8]]] = &[member_seeds];

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(MEMBER_MINT_FUNDED_SPACE);

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.member_mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MEMBER_MINT_SPACE,
        &ctx.accounts.token_program.address(),
        member_signer,
    )?;

    naclac_lang::prelude::initialize_group_member_pointer(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.member_mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        Some(&member_mint_address),
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.member_mint.to_cpi_handle_mut(),
        args.decimals,
        &mint_authority_address,
        None,
    )?;

    let bump = ctx.accounts.mint_authority.bump;
    let authority_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let authority_signer: &[&[&[u8]]] = &[authority_seeds];

    // `member` and `member_mint` are the same account (self-referential
    // `GroupMemberPointer`) — derive `member_mint_handle` from the mutable
    // handle's own `.info` rather than re-borrowing `ctx.accounts.member_mint`,
    // same reasoning as `create_mint2022_with_metadata_pointer_and_metadata.rs`.
    let member_handle = ctx.accounts.member_mint.to_cpi_handle_mut();
    let member_mint_info = member_handle.info.clone();
    let member_mint_handle = member_mint_info.to_cpi_handle();
    naclac_lang::prelude::initialize_token_group_member_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        naclac_lang::prelude::TokenGroupMemberInitializeAccounts {
            member: member_handle,
            member_mint: member_mint_handle,
            member_mint_authority: ctx.accounts.mint_authority.to_cpi_handle(),
            group: ctx.accounts.group_mint.to_cpi_handle_mut(),
            group_update_authority: ctx.accounts.mint_authority.to_cpi_handle(),
        },
        authority_signer,
    )?;

    Ok(())
}
