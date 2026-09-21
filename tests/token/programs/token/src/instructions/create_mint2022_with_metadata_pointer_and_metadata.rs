use naclac_lang::prelude::*;
use crate::components::MintAuthority;
use crate::constants::{SEED_MINT, SEED_MINT_AUTHORITY};

/// Real end-to-end proof of `initialize_token_metadata`
/// (`naclac-token/src/extensions/token_metadata.rs`): allocates the mint PDA
/// by hand, points a self-referential `MetadataPointer` at the mint itself
/// (required — Token-2022's own `TokenMetadata::Initialize` processor
/// rejects a metadata account that isn't the mint), initializes the mint,
/// then initializes `TokenMetadata` on it. Funds the account well beyond its
/// initial size: `TokenMetadata::Initialize`/`UpdateField` both grow the
/// account via `AccountInfo::resize` directly (no payer/`system_program`
/// account of their own), and `resize` never adds lamports — only pre-funded
/// headroom keeps a later grow rent-exempt.
#[instruction_args]
pub struct CreateMint2022WithMetadataPointerAndMetadataArgs {
    pub id: u64,
    pub mint_bump: u8,
    pub decimals: u8,
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

#[derive(Accounts)]
#[instruction(args: CreateMint2022WithMetadataPointerAndMetadataArgs)]
pub struct CreateMint2022WithMetadataPointerAndMetadata {
    #[account(mut)]
    pub payer: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below — there is no naclac `Discriminator` to check since this
    /// is a raw SPL `Mint` + `MetadataPointer` + `TokenMetadata` layout, not
    /// a naclac component.
    #[account(mut, seeds = [SEED_MINT, &args.id.to_le_bytes()], bump = args.mint_bump)]
    pub mint: AccountInfo,

    pub mint_authority: Account<MintAuthority>,

    pub token_program: Program<Token2022>,
    pub system_program: Program<System>,
}

/// Base 82 bytes + 83 bytes zero padding + 1-byte `AccountType` marker +
/// 4-byte TLV header + 64-byte `MetadataPointer` value — the account's data
/// length at `create_account` time. `TokenMetadata::Initialize` grows it
/// further itself once the mint is up.
const MINT_WITH_METADATA_POINTER_SPACE: u64 = 82 + 83 + 1 + 4 + 64;

/// Lamports are funded for this much larger target size up front — headroom
/// for the initial `TokenMetadata` TLV entry plus one later `UpdateField`
/// growth in the test, since neither CPI can add lamports mid-instruction.
const MINT_WITH_METADATA_POINTER_FUNDED_SPACE: usize = 600;

pub fn create_mint2022_with_metadata_pointer_and_metadata(
    ctx: Context<CreateMint2022WithMetadataPointerAndMetadata>,
    args: CreateMint2022WithMetadataPointerAndMetadataArgs,
) -> Result {
    let mint_authority_address = ctx.accounts.mint_authority.address();
    let mint_address = ctx.accounts.mint.address();
    let id_bytes = args.id.to_le_bytes();
    let mint_seeds: &[&[u8]] = &[SEED_MINT, &id_bytes, &[args.mint_bump]];
    let mint_signer: &[&[&[u8]]] = &[mint_seeds];

    let rent = Rent::get()?;
    let lamports = rent.try_minimum_balance(MINT_WITH_METADATA_POINTER_FUNDED_SPACE)?;

    system_program::create_account_signed(
        ctx.accounts.payer.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        lamports,
        MINT_WITH_METADATA_POINTER_SPACE,
        &ctx.accounts.token_program.address(),
        mint_signer,
    )?;

    naclac_lang::prelude::initialize_metadata_pointer(
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

    // `metadata` and `mint` are the same account (self-referential
    // `MetadataPointer`) — naclac's CPI handles forbid a live mutable and
    // immutable handle to the same account at once, so `mint_handle` is
    // built from a clone of the mutable handle's own `.info` rather than
    // re-borrowing `ctx.accounts.mint`.
    let metadata_handle = ctx.accounts.mint.to_cpi_handle_mut();
    let mint_info = metadata_handle.info;
    let mint_handle = mint_info.to_cpi_handle();
    let bump = ctx.accounts.mint_authority.bump;
    let authority_seeds: &[&[u8]] = &[SEED_MINT_AUTHORITY, &[bump]];
    let authority_signer: &[&[&[u8]]] = &[authority_seeds];
    naclac_lang::prelude::initialize_token_metadata_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        metadata_handle,
        ctx.accounts.mint_authority.to_cpi_handle(),
        mint_handle,
        ctx.accounts.mint_authority.to_cpi_handle(),
        naclac_lang::prelude::TokenMetadataInitializeParams {
            name: args.name,
            symbol: args.symbol,
            uri: args.uri,
        },
        authority_signer,
    )?;

    Ok(())
}
