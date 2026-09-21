use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::{SEED_MINT, SEED_MINT_AUTHORITY};

/// Real end-to-end proof of `initialize_token_group`
/// (`naclac-token/src/extensions/token_group.rs`): allocates the mint PDA by
/// hand, points a self-referential `GroupPointer` at the mint itself
/// (mirrors `TokenMetadata`'s own self-referential requirement), initializes
/// the mint, then initializes `TokenGroup` on it. Despite `TokenGroup` being
/// fixed-size, `InitializeGroup`'s own processor still grows the account
/// itself via the same self-reallocating `alloc_and_serialize` mechanism
/// `TokenMetadata::Initialize` uses (verified against
/// `spl-token-2022-11.0.0`'s real `extension::token_group::processor`) — so
/// the account must be created at exactly its `GroupPointer`-only size, not
/// pre-sized for `TokenGroup` too: `InitializeMint2`'s own validation
/// requires the account's length to exactly equal
/// `ExtensionType::try_calculate_account_len` for whatever extensions are
/// present *at that point* (just `GroupPointer`), rejecting any extra
/// pre-allocated headroom as `InvalidAccountData`.
#[instruction_args]
pub struct CreateMint2022WithGroupPointerAndGroupArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
    pub max_size: u64,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithGroupPointerAndGroupArgs)]
pub struct CreateMint2022WithGroupPointerAndGroup {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below — there is no naclac `Discriminator` to check since this
    /// is a raw SPL `Mint` + `GroupPointer` + `TokenGroup` layout, not a
    /// naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 + 83 padding + 1-byte `AccountType` + 4-byte `GroupPointer` TLV
/// header + 64-byte `GroupPointer` value — the account's data length at
/// `create_account` time. `InitializeGroup` grows it further itself once
/// the mint is up (see this file's own doc comment).
const MINT_WITH_GROUP_POINTER_SPACE: u64 = 82 + 83 + 1 + 4 + 64;

/// Lamports are funded for this much larger target size up front, rounded
/// up generously from `82 + 83 + 1 + 4 + 64` (`GroupPointer`) plus `4 + 80`
/// (`TokenGroup` TLV header plus `size_of::<TokenGroup>()`: 32
/// `update_authority`, 32 `mint`, 8 `size`, 8 `max_size`), i.e. 318 total —
/// `resize` inside `InitializeGroup` never adds lamports, only pre-funded
/// headroom keeps the grow rent-exempt.
const MINT_WITH_GROUP_POINTER_AND_GROUP_FUNDED_SPACE: usize = 400;

pub fn create_mint2022_with_group_pointer_and_group(
    ctx: Context<CreateMint2022WithGroupPointerAndGroup>,
    args: CreateMint2022WithGroupPointerAndGroupArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let mint_address = ctx.accounts.mint.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(MINT_WITH_GROUP_POINTER_AND_GROUP_FUNDED_SPACE);

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_GROUP_POINTER_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_group_pointer(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        Some(&mint_address),
    )?;

    naclac_lang::prelude::initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        args.decimals,
        &mint_authority_address,
        None,
    )?;

    // `group` and `mint` are the same account (self-referential
    // `GroupPointer`) — derive `mint_handle` from the mutable handle's own
    // `.info` rather than re-borrowing `ctx.accounts.mint`, same reasoning
    // as `create_mint2022_with_metadata_pointer_and_metadata.rs`.
    let group_handle = ctx.accounts.mint.to_cpi_handle_mut();
    let mint_info = group_handle.info.clone();
    let mint_handle = mint_info.to_cpi_handle();
    let bump = ctx.accounts.mint_authority.bump;
    let authority_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let authority_signer: &[&[&[u8]]] = &[authority_seeds];
    naclac_lang::prelude::initialize_token_group_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        group_handle,
        mint_handle,
        ctx.accounts.mint_authority.to_cpi_handle(),
        Some(&mint_authority_address),
        args.max_size,
        authority_signer,
    )?;

    Ok(())
}
