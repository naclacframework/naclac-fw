use naclac_lang::prelude::*;
use naclac_lang::prelude::spl_token_metadata_interface;
use crate::components::{BondingCurve, Global};
use crate::constants::{BONDING_CURVE_SEED, GLOBAL_SEED, MINT_AUTHORITY_SEED, MINT_DECIMALS, WSOL_MINT};
use crate::errors::PumpError;
use crate::events::CreateEvent;

// Real `create_v2` account list + args confirmed via `pump-rust-client`'s own
// IDL (`reference/pump-rust-client/idls/pump.json`, discriminator
// [214,144,76,236,95,139,49,180]) and cross-checked byte-for-byte against 4
// real mainnet transactions' scoped inner instructions, then independently
// re-verified against real, deployed `pump.so` bytecode directly in litesvm
// (`reference/fee-tier-probe/src/bin/probe55.rs`). Confirmed real facts this
// implementation depends on:
// - Real `create_v2` always mints a Token-2022 coin with a self-referential
//   `MetadataPointer` (authority/metadata_address = the mint itself) plus a
//   real `TokenMetadata` (name/symbol/uri) initialized via the SPL Token
//   Metadata Interface — not Metaplex, unlike classic `create`.
// - Real inner-CPI order: `CreateAccount(mint, space=234)` (82 base + 83
//   zero-pad + 1 `AccountType` marker + 4 `MetadataPointer` TLV header + 64
//   value) → `MetadataPointerExtension::Initialize` → `InitializeMint2` →
//   [bonding_curve/associated_bonding_curve creation] → a second `Transfer`
//   funding the mint up to whatever `TokenMetadata`'s own real TLV size
//   requires → `TokenMetadata::Initialize` → `TokenMetadata::UpdateAuthority`
//   (real value: `None` — create_v2 always revokes metadata update authority
//   immediately after setting it) → `MintTo` → `SetAuthority(MintTokens,
//   None)` (real: mint authority is also permanently revoked after minting
//   the full supply, unlike classic `create`, which keeps `mint_authority`
//   live).
// - `is_mayhem_mode`/the 5 real mayhem-only accounts (`mayhem_program_id`,
//   `global_params`, `sol_vault`, `mayhem_state`, `mayhem_token_vault`) are
//   dropped entirely from this reimplementation, per this program's standing
//   decision. Confirmed safe via `probe55.rs`: real `pump.so`'s `create_v2`
//   succeeds even when
//   `mayhem_token_vault` is passed as a completely arbitrary, never-before-
//   seen address — none of the 5 mayhem accounts are read or written at all
//   when `is_mayhem_mode=false`, confirmed both from 3 independent real
//   mainnet transactions' scoped inner instructions and from this live probe.
// - `is_cashback_enabled` is real-protocol-typed `OptionBool`, but that type
//   is (confirmed elsewhere this session) a plain required `bool` with no
//   `None` state — 1 byte on the wire, identical to naclac's own `Bool`.
// - Non-SOL quote mint support (`pump-public-docs/docs/instructions/COIN_CREATION.md`):
//   real `create_v2` takes 3 optional accounts (`quote_mint`,
//   `associated_quote_bonding_curve`, `quote_token_program`) as
//   `remaining_accounts`, "all three or none" — not declared IDL accounts.
//   Re-expressed here as naclac's own declarative `Option<T>` account fields
//   instead (`naclac-macros/docs/optional-accounts-plan.md`) rather than
//   reading `ctx.remaining_accounts` directly, so the generated client SDK
//   gets real typed, IDL-visible support for this capability instead of a
//   hand-built `AccountMeta` side channel every caller would otherwise have
//   to reinvent. Confirmed directly against real `pump.so`
//   (`reference/fee-tier-probe/src/bin/probe64.rs`), not the docs alone,
//   since the docs and the real IDL's own error enum disagree on one point:
//   `quote_token_program` must be the legacy SPL Token program exactly —
//   passing Token-2022 hits the real, live `InvalidQuoteTokenProgram` (6064)
//   check, contradicting `COIN_CREATION.md`'s "not necessarily legacy Token"
//   wording. Also confirmed via the same probe: a non-SOL-paired curve's
//   `bonding_curve.virtual_quote_reserves` seeds from
//   `Global.initial_virtual_quote_reserves` (not `initial_virtual_sol_reserves`,
//   the SOL-only path), and real `create_v2` itself creates
//   `associated_quote_bonding_curve` (not the caller).
//
// The mint below is created via three explicit CPIs
// (`system_program::create_account` + `initialize_metadata_pointer` +
// `initialize_mint`) rather than the declarative `mint::decimals`/
// `mint::authority` sugar `create.rs` uses: naclac-macros' `init` attribute
// parser (`naclac-macros/src/instruction/parser.rs`) only recognizes
// `mint::decimals`/`mint::authority`/`mint::freeze_authority` today, with no
// `extensions::*` keys at all, so there's no declarative way to sandwich a
// Token-2022 extension-initialization CPI between allocation and
// `InitializeMint2`. This mirrors the exact pattern naclac-token's own test
// suite already uses for this same scenario
// (`tests/token/programs/token_borsh/src/instructions/create_mint2022_with_metadata_pointer_and_metadata.rs`).
#[derive(Accounts)]
#[instruction(name: ZcString, symbol: ZcString, uri: ZcString, creator: Address, is_cashback_enabled: Bool, bonding_curve_bump: u8)]
pub struct CreateV2 {
    #[account(mut)]
    pub user: Signer,

    /// SAFETY: allocated and initialized entirely by hand in the handler
    /// body below — see the module comment for why the declarative `mint::`
    /// sugar can't be used here.
    #[account(mut)]
    pub mint: Signer,

    /// SAFETY: the `seeds`/`bump` constraint fully validates this; it's a
    /// lamport-only PDA (no stored data) — the mint's authority throughout
    /// creation, signed via its own seeds for every CPI below.
    #[account(seeds = [MINT_AUTHORITY_SEED], bump)]
    pub mint_authority: AccountInfo,

    #[account(
        init,
        payer = user,
        seeds = [BONDING_CURVE_SEED, mint.address().as_ref()],
        bump = bonding_curve_bump,
    )]
    pub bonding_curve: Account<BondingCurve>,

    /// SAFETY: created by hand in the handler body below via
    /// `associated_token::create`, after the mint is fully initialized — see
    /// the module comment for why. The real Associated Token Program CPI
    /// itself derives and enforces the canonical ATA address for
    /// `(bonding_curve, token_program, mint)` server-side, the same
    /// enforcement the declarative `associated_token::...` sugar (used by
    /// classic `create.rs`) ultimately relies on too.
    #[account(mut)]
    pub associated_bonding_curve: AccountInfo,

    #[account(seeds = [GLOBAL_SEED], bump)]
    pub global: Account<Global>,

    pub system_program: Program<System>,
    pub token_program: Program<Token2022>,
    pub associated_token_program: Program<AssociatedToken>,

    /// Non-SOL quote mint, "all three or none" together with
    /// `quote_token_program`/`associated_quote_bonding_curve` below — see the
    /// module comment. `None` (real create_v2's remaining-accounts omitted)
    /// means SOL-paired, same as passing WSOL explicitly.
    #[account(mut)]
    pub quote_mint: Option<InterfaceAccount<Mint>>,

    /// SAFETY: only ever compared against `TOKEN_PROGRAM_ID` in the handler
    /// body, never deserialized — real `create_v2` requires classic Token
    /// specifically here (confirmed via `probe64.rs`, `Custom(6064)`
    /// `InvalidQuoteTokenProgram` for Token-2022), so this stays a raw
    /// `AccountInfo` rather than a typed `Program<Token>` to preserve that
    /// exact real error instead of a generic constraint-mismatch one.
    pub quote_token_program: Option<AccountInfo>,

    /// SAFETY: created by hand in the handler body below, mirroring
    /// `associated_bonding_curve` above — confirmed real `create_v2` creates
    /// this itself (non-idempotent `Create`, `probe64.rs`), never
    /// deserialized before that point.
    #[account(mut)]
    pub associated_quote_bonding_curve: Option<AccountInfo>,
}

#[instruction]
pub fn create_v2(
    ctx: Context<CreateV2>,
    name: ZcString,
    symbol: ZcString,
    uri: ZcString,
    creator: Address,
    is_cashback_enabled: Bool,
    _bonding_curve_bump: u8,
) -> Result {
    let global = &ctx.accounts.global;
    require!(bool::from(global.create_v2_enabled), PumpError::CreateV2Disabled);

    // Always `false` here -- mayhem mode itself is out of scope for this
    // reimplementation (see the module comment), never a caller-supplied
    // arg. Checked anyway, in the real protocol's own shape, so this stays
    // structurally correct if mayhem mode support is ever added later; it
    // can never actually fail while this local stays hardcoded `false`.
    let is_mayhem_mode = Bool::from(false);
    require!(
        !bool::from(is_mayhem_mode) || bool::from(global.mayhem_mode_enabled),
        PumpError::MayhemModeDisabled
    );
    require!(
        !bool::from(is_cashback_enabled) || bool::from(global.is_cashback_enabled),
        PumpError::CashbackNotEnabled
    );

    let token_total_supply = global.token_total_supply;
    let virtual_token_reserves = global.initial_virtual_token_reserves;
    let virtual_sol_reserves = global.initial_virtual_sol_reserves;
    let real_token_reserves = global.initial_real_token_reserves;

    ctx.accounts.bonding_curve.creator = creator;
    ctx.accounts.bonding_curve.virtual_token_reserves = virtual_token_reserves;
    ctx.accounts.bonding_curve.real_token_reserves = real_token_reserves;
    ctx.accounts.bonding_curve.token_total_supply = token_total_supply;
    ctx.accounts.bonding_curve.is_mayhem_mode = is_mayhem_mode;
    ctx.accounts.bonding_curve.is_cashback_coin = is_cashback_enabled;

    let mint_authority_address = ctx.accounts.mint_authority.address();
    let mint_address = ctx.accounts.mint.address();
    let mint_authority_signer_seeds: &[&[u8]] = &[MINT_AUTHORITY_SEED, &[ctx.bumps.mint_authority]];
    let mint_authority_signer: &[&[&[u8]]] = &[mint_authority_signer_seeds];

    // Base size for a Token-2022 mint carrying a self-referential
    // `MetadataPointer`: 82 (base `Mint`) + 83 zero-pad up to the shared
    // extension-region offset (165) + 1 `AccountType` marker + 4 TLV header
    // + 64 `MetadataPointer` value. Matches `pump.so`'s own real
    // `CreateAccount` call exactly (`probe55.rs`).
    const MINT_WITH_METADATA_POINTER_SPACE: u64 = 82 + 83 + 1 + 4 + 64;

    let rent = Rent::get()?;
    let initial_lamports = rent.try_minimum_balance(MINT_WITH_METADATA_POINTER_SPACE as usize)?;

    system_program::create_account(
        ctx.accounts.user.to_cpi_handle_mut(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.system_program.to_cpi_handle(),
        initial_lamports,
        MINT_WITH_METADATA_POINTER_SPACE,
        &ctx.accounts.token_program.address(),
    )?;

    initialize_metadata_pointer(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        Some(&mint_authority_address),
        Some(&mint_address),
    )?;

    initialize_mint(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        MINT_DECIMALS,
        &mint_authority_address,
        None,
    )?;

    // `associated_bonding_curve` is created here, after the mint is fully
    // initialized, rather than via the declarative `associated_token::init`
    // sugar `create.rs` uses — the real Associated Token Program CPI reads
    // the mint's own account data (`GetAccountDataSize`) to size the ATA
    // under Token-2022, which fails if attempted before `initialize_mint`
    // above has run (naclac's declarative `init` fields are created at
    // account-validation time, before this handler body — too early here).
    ctx.accounts.associated_token_program.create(CreateAtaAccounts {
        payer: &mut ctx.accounts.user,
        associated_token: &mut ctx.accounts.associated_bonding_curve,
        authority: &ctx.accounts.bonding_curve,
        mint: &ctx.accounts.mint,
        system_program: &ctx.accounts.system_program,
        token_program: &ctx.accounts.token_program,
    })?;

    // `TokenMetadata::Initialize`/`UpdateField` grow the mint account via
    // `AccountInfo::resize` directly, with no payer/`system_program` account
    // of their own — the account must already hold enough lamports to stay
    // rent-exempt at its final size, funded up front here via the real
    // interface crate's own `tlv_size_of` (not a hand-derived estimate).
    let metadata_tlv_size = spl_token_metadata_interface::state::TokenMetadata {
        mint: *mint_address.as_address(),
        name: String::from(name.as_str()),
        symbol: String::from(symbol.as_str()),
        uri: String::from(uri.as_str()),
        ..Default::default()
    }
    .tlv_size_of()?;
    let final_mint_size = MINT_WITH_METADATA_POINTER_SPACE as usize + metadata_tlv_size;
    let final_lamports = rent.try_minimum_balance(final_mint_size)?;
    let mint_lamports_now = ctx.accounts.mint.lamports();
    if final_lamports > mint_lamports_now {
        ctx.accounts.system_program.transfer(
            SystemTransferAccounts { from: &mut ctx.accounts.user, to: &mut ctx.accounts.mint },
            final_lamports - mint_lamports_now,
        )?;
    }

    // `metadata` and `mint` are the same account (self-referential
    // `MetadataPointer`) — naclac's CPI handles forbid a live mutable and
    // immutable handle to the same account at once, so the immutable `mint`
    // handle below is built from a clone of the mutable `metadata` handle's
    // own `.info` rather than re-borrowing `ctx.accounts.mint`.
    let metadata_handle = ctx.accounts.mint.to_cpi_handle_mut();
    let mint_info_clone = metadata_handle.info;
    let mint_handle = mint_info_clone.to_cpi_handle();
    initialize_token_metadata_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        metadata_handle,
        ctx.accounts.mint_authority.to_cpi_handle(),
        mint_handle,
        ctx.accounts.mint_authority.to_cpi_handle(),
        TokenMetadataInitializeParams {
            name: String::from(name.as_str()),
            symbol: String::from(symbol.as_str()),
            uri: String::from(uri.as_str()),
        },
        mint_authority_signer,
    )?;

    update_token_metadata_authority_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        None,
        mint_authority_signer,
    )?;

    ctx.accounts.token_program.mint_to_signed(
        MintToAccounts {
            mint: &mut ctx.accounts.mint,
            to: &mut ctx.accounts.associated_bonding_curve,
            authority: &ctx.accounts.mint_authority,
        },
        token_total_supply,
        mint_authority_signer,
    )?;

    set_authority_signed(
        ctx.accounts.token_program.to_cpi_handle(),
        ctx.accounts.mint.to_cpi_handle_mut(),
        ctx.accounts.mint_authority.to_cpi_handle(),
        AuthorityType::MintTokens,
        None,
        mint_authority_signer,
    )?;

    // Optional non-SOL quote mint: `quote_mint`/`quote_token_program`/
    // `associated_quote_bonding_curve` must be all-`Some` or all-`None`
    // together — see the module comment. Checked last, matching the real
    // program's own confirmed ordering (`probe64.rs`: the failure case shows
    // every base-mint CPI already completed before this check runs).
    let all_present = ctx.accounts.quote_mint.is_some()
        && ctx.accounts.quote_token_program.is_some()
        && ctx.accounts.associated_quote_bonding_curve.is_some();
    let all_absent = ctx.accounts.quote_mint.is_none()
        && ctx.accounts.quote_token_program.is_none()
        && ctx.accounts.associated_quote_bonding_curve.is_none();
    require!(all_present || all_absent, PumpError::NotEnoughRemainingAccounts);

    let (quote_mint, virtual_quote_reserves) = if !all_present {
        (Address::default(), virtual_sol_reserves)
    } else {
        // Real `create_v2` only ever accepts the legacy Token program here,
        // never Token-2022 — confirmed via `probe64.rs` against real
        // `pump.so` (`Custom(6064)`, `InvalidQuoteTokenProgram`), which
        // contradicts `COIN_CREATION.md`'s own prose on this point.
        let quote_token_program_address = ctx.accounts.quote_token_program.as_ref().unwrap().address();
        require!(
            quote_token_program_address == TOKEN_PROGRAM_ID,
            PumpError::InvalidQuoteTokenProgram
        );

        let quote_mint_address = ctx.accounts.quote_mint.as_ref().unwrap().address();
        if quote_mint_address == WSOL_MINT {
            (Address::default(), virtual_sol_reserves)
        } else {
            require!(
                global.whitelisted_quote_mints.contains(&quote_mint_address),
                PumpError::QuoteMintNotWhitelisted
            );

            ctx.accounts.associated_token_program.create(CreateAtaAccounts {
                payer: &mut ctx.accounts.user,
                associated_token: ctx.accounts.associated_quote_bonding_curve.as_mut().unwrap(),
                authority: &ctx.accounts.bonding_curve,
                mint: ctx.accounts.quote_mint.as_ref().unwrap(),
                system_program: &ctx.accounts.system_program,
                token_program: ctx.accounts.quote_token_program.as_ref().unwrap(),
            })?;

            (quote_mint_address, global.initial_virtual_quote_reserves)
        }
    };

    ctx.accounts.bonding_curve.quote_mint = quote_mint;
    ctx.accounts.bonding_curve.virtual_quote_reserves = virtual_quote_reserves;

    let timestamp = unix_timestamp()?;
    emit!(CreateEvent {
        name: String::from(name.as_str()),
        symbol: String::from(symbol.as_str()),
        uri: String::from(uri.as_str()),
        mint: mint_address,
        bonding_curve: ctx.accounts.bonding_curve.address(),
        user: ctx.accounts.user.address(),
        creator,
        timestamp,
        virtual_token_reserves,
        virtual_sol_reserves,
        real_token_reserves,
        token_total_supply,
        token_program: ctx.accounts.token_program.address(),
        is_mayhem_mode,
        is_cashback_enabled,
        quote_mint,
        virtual_quote_reserves,
    });

    msg!("Coin successfully created");
    Ok(())
}
