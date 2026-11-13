//! Token and Mint account layouts: typed readers over the raw SPL Token bytes,
//! the constraint checks behind the `token::*`/`mint::*` attributes, and the
//! layout validation they share.

use crate::{
    prelude::{Address, Discriminator, NaclacError},
    wrappers::TokenInterface,
};

#[cfg_attr(
    feature = "borsh",
    derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize)
)]
#[cfg_attr(feature = "borsh", borsh(crate = "crate::borsh"))]
#[cfg_attr(
    feature = "wincode",
    derive(crate::wincode::SchemaRead, crate::wincode::SchemaWrite)
)]
#[cfg_attr(feature = "wincode", wincode(crate = "crate::wincode"))]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TokenAccount(pub [u8; 165]);

#[cfg_attr(
    feature = "borsh",
    derive(crate::borsh::BorshSerialize, crate::borsh::BorshDeserialize)
)]
#[cfg_attr(feature = "borsh", borsh(crate = "crate::borsh"))]
#[cfg_attr(
    feature = "wincode",
    derive(crate::wincode::SchemaRead, crate::wincode::SchemaWrite)
)]
#[cfg_attr(feature = "wincode", wincode(crate = "crate::wincode"))]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Mint(pub [u8; 82]);

// SAFETY: TokenAccount is a transparent wrapper over [u8; 165], which is a POD type.
unsafe impl crate::prelude::Pod for TokenAccount {}
// SAFETY: TokenAccount is a transparent wrapper over [u8; 165], which can be safely zero-initialized.
unsafe impl crate::prelude::Zeroable for TokenAccount {}

// SAFETY: Mint is a transparent wrapper over [u8; 82], which is a POD type.
unsafe impl crate::prelude::Pod for Mint {}
// SAFETY: Mint is a transparent wrapper over [u8; 82], which can be safely zero-initialized.
unsafe impl crate::prelude::Zeroable for Mint {}

impl Discriminator for TokenAccount {
    const DISCRIMINATOR: &'static [u8] = &[];
    const PROGRAM_OWNED: bool = false;

    /// A plain `TokenAccount` is always owned by the legacy Token program.
    /// `InterfaceAccount<TokenAccount>` (below) is the type to use for a
    /// field that must also accept Token-2022. Beyond the owner check, also
    /// validates the raw layout (exact length, canonical `COption` tags,
    /// initialized state) via `validate_token_account_bytes`.
    fn validate_account(
        info: &crate::prelude::AccountInfo,
        index: usize,
    ) -> crate::prelude::Result<()> {
        if crate::prelude::Owner::program_owner(info) != crate::prelude::TOKEN_PROGRAM_ID {
            return Err(crate::prelude::NaclacError::ConstraintOwner.err(index));
        }
        validate_token_account_bytes(info, index, true)
    }
}

impl Discriminator for Mint {
    const DISCRIMINATOR: &'static [u8] = &[];
    const PROGRAM_OWNED: bool = false;

    /// See `TokenAccount::validate_account` above; validates the `Mint`
    /// layout via `validate_mint_bytes`.
    fn validate_account(
        info: &crate::prelude::AccountInfo,
        index: usize,
    ) -> crate::prelude::Result<()> {
        if crate::prelude::Owner::program_owner(info) != crate::prelude::TOKEN_PROGRAM_ID {
            return Err(crate::prelude::NaclacError::ConstraintOwner.err(index));
        }
        validate_mint_bytes(info, index, true)
    }
}

#[cfg(feature = "bytemuck")]
impl crate::prelude::NaclacZeroCopy for TokenAccount {}
#[cfg(feature = "bytemuck")]
impl crate::prelude::NaclacZeroCopy for Mint {}

#[cfg(feature = "idl-build")]
impl naclac_idl::idl_build::NaclacIdlBuild for TokenAccount {
    fn get_full_path() -> naclac_idl::__private::String {
        naclac_idl::__private::String::from("TokenAccount")
    }
}

#[cfg(feature = "idl-build")]
impl naclac_idl::idl_build::NaclacIdlBuild for Mint {
    fn get_full_path() -> naclac_idl::__private::String {
        naclac_idl::__private::String::from("Mint")
    }
}

/// `InterfaceAccount<TokenAccount>`/`InterfaceAccount<Mint>` (naclac-core's
/// `wrappers/accounts/interface_account.rs`) accept either the legacy Token
/// program or Token-2022 owner — used directly on `TokenAccount`/`Mint`,
/// with no separate interface-named type, matching real Anchor's own
/// `InterfaceAccount<TokenAccount>` naming exactly. Beyond the owner check
/// naclac-core's `InterfaceAccount<T>` performs itself,
/// `ValidateInterfaceLayout` validates the raw layout permissively (`>=`
/// minimum length, since Token-2022 extensions append a TLV region after
/// the base structure) — canonical `COption` tags, initialized state — via
/// `validate_token_account_layout`/`validate_mint_layout` with
/// `exact_len: false`.
impl crate::prelude::ValidateInterfaceLayout for TokenAccount {
    const OWNERS: &'static [Address] = TokenInterface::IDS;

    fn validate_interface_layout(
        data: &[u8],
    ) -> core::result::Result<(), crate::prelude::NaclacError> {
        validate_token_account_layout(data, false)
    }
}

impl crate::prelude::ValidateInterfaceLayout for Mint {
    const OWNERS: &'static [Address] = TokenInterface::IDS;

    fn validate_interface_layout(
        data: &[u8],
    ) -> core::result::Result<(), crate::prelude::NaclacError> {
        validate_mint_layout(data, false)
    }
}

/// Reads a `COption<Address>` at `tag_offset` (4-byte tag, then 32-byte
/// value immediately after) — `None` unless the tag is exactly the
/// canonical `[1,0,0,0]` `Some` encoding. Shared by `TokenAccount`'s
/// `delegate`/`close_authority` and `Mint`'s `mint_authority`/
/// `freeze_authority`, all of which use this exact layout.
fn read_coption_address(data: &[u8], tag_offset: usize) -> Option<Address> {
    if data[tag_offset..tag_offset + 4] != [1u8, 0, 0, 0] {
        return None;
    }
    let bytes: [u8; 32] = data[tag_offset + 4..tag_offset + 36]
        .try_into()
        .unwrap_or_else(|_| panic!("slice length mismatch"));
    Some(Address::new_from_array(bytes))
}

impl TokenAccount {
    pub fn mint(&self) -> Address {
        let bytes = unsafe { *(self.0.as_ptr() as *const [u8; 32]) };
        Address::new_from_array(bytes)
    }
    pub fn owner(&self) -> Address {
        let bytes = unsafe { *(self.0.as_ptr().add(32) as *const [u8; 32]) };
        Address::new_from_array(bytes)
    }
    pub fn amount(&self) -> u64 {
        let bytes = unsafe { *(self.0.as_ptr().add(64) as *const [u8; 8]) };
        u64::from_le_bytes(bytes)
    }
    /// The delegate authorized to transfer up to `delegated_amount()`, if any.
    pub fn delegate(&self) -> Option<Address> {
        read_coption_address(&self.0, 72)
    }
    /// Raw SPL `AccountState` byte: `0` = uninitialized, `1` = initialized, `2` = frozen.
    pub fn state(&self) -> u8 {
        self.0[108]
    }
    /// `true` if `state() == 2` (frozen) — a frozen account rejects `transfer`/`burn`.
    pub fn is_frozen(&self) -> bool {
        self.state() == 2
    }
    /// The rent-exempt reserve if this is a wrapped-SOL account, else `None`.
    pub fn is_native(&self) -> Option<u64> {
        if self.0[109..113] != [1u8, 0, 0, 0] {
            return None;
        }
        let bytes: [u8; 8] = self.0[113..121]
            .try_into()
            .unwrap_or_else(|_| panic!("slice length mismatch"));
        Some(u64::from_le_bytes(bytes))
    }
    /// The amount `delegate()` is currently authorized to transfer, if any delegate is set.
    pub fn delegated_amount(&self) -> u64 {
        let bytes: [u8; 8] = self.0[121..129]
            .try_into()
            .unwrap_or_else(|_| panic!("slice length mismatch"));
        u64::from_le_bytes(bytes)
    }
    /// The authority allowed to close this account and reclaim its rent, if any.
    pub fn close_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 129)
    }
}

impl Mint {
    pub fn supply(&self) -> u64 {
        let bytes = unsafe { *(self.0.as_ptr().add(36) as *const [u8; 8]) };
        u64::from_le_bytes(bytes)
    }
    pub fn decimals(&self) -> u8 {
        self.0[44]
    }
    /// The authority allowed to mint new tokens, if the supply isn't fixed.
    pub fn mint_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 0)
    }
    /// The authority allowed to freeze/thaw token accounts of this mint, if any.
    pub fn freeze_authority(&self) -> Option<Address> {
        read_coption_address(&self.0, 46)
    }
    /// `true` once the mint has been initialized via `InitializeMint`/`InitializeMint2`.
    pub fn is_initialized(&self) -> bool {
        self.0[45] != 0
    }
}

/// Raw-byte constraint checks backing the `token::*`/`mint::*`/
/// `associated_token::*` attributes in `#[account(...)]`. Each function takes
/// the account's raw data slice rather than a hydrated `&TokenAccount`/`&Mint`,
/// since the generated constraint code runs immediately on `AccountInfo` data,
/// before (and independent of) any typed zero-copy load. This is the single
/// place naclac-derive' codegen defers to for the actual SPL Token/Token-2022
/// layout knowledge — the macro only resolves which expression to compare
/// against and emits the call.
impl TokenAccount {
    /// Compares the account's mint field (offset 0..32) to `expected`.
    pub fn check_mint(data: &[u8], expected: &Address) -> core::result::Result<(), NaclacError> {
        if data.len() < 32 {
            return Err(NaclacError::AccountDataTooSmall);
        }
        let actual = Address::new_from_array(
            data[0..32]
                .try_into()
                .unwrap_or_else(|_| panic!("slice length mismatch")),
        );
        if &actual != expected {
            return Err(NaclacError::ConstraintTokenMint);
        }
        Ok(())
    }

    /// Compares the account's owner/authority field (offset 32..64) to `expected`.
    pub fn check_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 64 {
            return Err(NaclacError::AccountDataTooSmall);
        }
        let actual = Address::new_from_array(
            data[32..64]
                .try_into()
                .unwrap_or_else(|_| panic!("slice length mismatch")),
        );
        if &actual != expected {
            return Err(NaclacError::ConstraintTokenAuthority);
        }
        Ok(())
    }
}

impl Mint {
    /// Compares the mint's decimals byte (offset 44) to `expected`.
    pub fn check_decimals(data: &[u8], expected: u8) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::AccountDataTooSmall);
        }
        if data[44] != expected {
            return Err(NaclacError::ConstraintMintDecimals);
        }
        Ok(())
    }

    /// Reads the mint-authority `COption` (tag at 0..4, address at 4..36):
    /// errors if the authority is `None`, else compares the address to `expected`.
    pub fn check_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::AccountDataTooSmall);
        }
        if data[0..4] != [1u8, 0, 0, 0] {
            return Err(NaclacError::Unauthorized);
        }
        let actual = Address::new_from_array(
            data[4..36]
                .try_into()
                .unwrap_or_else(|_| panic!("slice length mismatch")),
        );
        if &actual != expected {
            return Err(NaclacError::ConstraintMintAuthority);
        }
        Ok(())
    }

    /// Reads the freeze-authority `COption` (tag at 46..50, address at 50..82):
    /// errors if the freeze authority is `None`, else compares the address to `expected`.
    pub fn check_freeze_authority(
        data: &[u8],
        expected: &Address,
    ) -> core::result::Result<(), NaclacError> {
        if data.len() < 82 {
            return Err(NaclacError::AccountDataTooSmall);
        }
        if data[46..50] != [1u8, 0, 0, 0] {
            return Err(NaclacError::Unauthorized);
        }
        let actual = Address::new_from_array(
            data[50..82]
                .try_into()
                .unwrap_or_else(|_| panic!("slice length mismatch")),
        );
        if &actual != expected {
            return Err(NaclacError::ConstraintMintFreezeAuthority);
        }
        Ok(())
    }
}

/// True for the two `COption` tag encodings the real SPL Token program ever
/// writes — `[0,0,0,0]` (`None`) or `[1,0,0,0]` (`Some`). Any other 4-byte
/// pattern cannot come from real SPL Token/Token-2022 account data.
fn is_canonical_coption_tag(tag: &[u8]) -> bool {
    tag == [0u8, 0, 0, 0] || tag == [1u8, 0, 0, 0]
}

/// Validates the fixed 165-byte SPL Token `Account` layout beyond
/// owner/discriminator: length (exactly 165 when `exact_len`, else `>= 165`
/// — a Token-2022 account with extensions is legitimately *longer* than 165
/// bytes, a TLV region appended after the base structure, so
/// `InterfaceAccount<TokenAccount>` must not demand exact length the way
/// plain `TokenAccount` does; confirmed against anchor-spl-v2's own split:
/// `token::TokenAccount::validate` uses `require_eq!`, `Interface<TokenAccount>::validate`
/// does not), canonical `COption` tag encoding on
/// `delegate`/`is_native`/`close_authority` (offsets verified against
/// `spl-token-interface`'s real `Account` struct layout, not assumed), and a
/// `state` byte of `Initialized` (1) or `Frozen` (2) — `Uninitialized` (0)
/// is reported distinctly from any other out-of-range value, matching
/// anchor-spl-v2's `validate_token_account_initialized`.
fn validate_token_account_layout(
    data: &[u8],
    exact_len: bool,
) -> core::result::Result<(), NaclacError> {
    let len_ok = if exact_len {
        data.len() == 165
    } else {
        data.len() >= 165
    };
    if !len_ok {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    if !is_canonical_coption_tag(&data[72..76]) // delegate
        || !is_canonical_coption_tag(&data[109..113]) // is_native
        || !is_canonical_coption_tag(&data[129..133])
    // close_authority
    {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    match data[108] {
        0 => Err(NaclacError::AccountNotInitialized),
        1 | 2 => Ok(()),
        _ => Err(NaclacError::InvalidAccountDiscriminator),
    }
}

/// Validates the fixed 82-byte SPL Token `Mint` layout beyond
/// owner/discriminator: length (exactly 82 when `exact_len`, else `>= 82` —
/// see `validate_token_account_layout`'s doc for why `InterfaceAccount<Mint>`
/// needs the non-exact mode), canonical `COption` tag encoding on
/// `mint_authority`/`freeze_authority`, and an `is_initialized` byte of
/// exactly `1` — `0` is reported distinctly, matching
/// `validate_token_account_layout` above.
fn validate_mint_layout(data: &[u8], exact_len: bool) -> core::result::Result<(), NaclacError> {
    let len_ok = if exact_len {
        data.len() == 82
    } else {
        data.len() >= 82
    };
    if !len_ok {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    if !is_canonical_coption_tag(&data[0..4]) // mint_authority
        || !is_canonical_coption_tag(&data[46..50])
    // freeze_authority
    {
        return Err(NaclacError::InvalidAccountDiscriminator);
    }
    match data[45] {
        0 => Err(NaclacError::AccountNotInitialized),
        1 => Ok(()),
        _ => Err(NaclacError::InvalidAccountDiscriminator),
    }
}

/// Borrows `info`'s raw data and runs `validate_token_account_layout`,
/// converting any failure to a `ProgramError` at `index`. Backend-specific
/// data access only; the actual byte validation is backend-independent.
fn validate_token_account_bytes(
    info: &crate::prelude::AccountInfo,
    index: usize,
    exact_len: bool,
) -> crate::prelude::Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = info.try_borrow_data()?;
        validate_token_account_layout(&data, exact_len).map_err(|e| e.err(index))
    }
    #[cfg(feature = "pinocchio")]
    {
        validate_token_account_layout(info.data(), exact_len).map_err(|e| e.err(index))
    }
}

/// Like `validate_token_account_bytes`, for `validate_mint_layout`.
fn validate_mint_bytes(
    info: &crate::prelude::AccountInfo,
    index: usize,
    exact_len: bool,
) -> crate::prelude::Result<()> {
    #[cfg(not(feature = "pinocchio"))]
    {
        let data = info.try_borrow_data()?;
        validate_mint_layout(&data, exact_len).map_err(|e| e.err(index))
    }
    #[cfg(feature = "pinocchio")]
    {
        validate_mint_layout(info.data(), exact_len).map_err(|e| e.err(index))
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// `read_coption_address` panics by design when `data` is too short for
    /// a 36-byte window at `tag_offset` — same "safe within its real
    /// contract" shape used throughout this audit. Every real call site
    /// passes a fixed, statically in-bounds offset into an already
    /// length-checked `TokenAccount`/`Mint` buffer.
    #[kani::proof]
    fn prove_read_coption_address_within_contract_never_panics() {
        let data: [u8; 165] = kani::any();
        let tag_offset: usize = kani::any();
        kani::assume(tag_offset <= data.len() - 36);
        let _ = read_coption_address(&data, tag_offset);
    }

    /// Correctness: `read_coption_address` must return `None` only for the
    /// canonical `[0,0,0,0]` tag, `Some` with the real 32 bytes for the
    /// canonical `[1,0,0,0]` tag, and `None` for any other (non-canonical,
    /// malformed) tag value too — the function's own doc comment says it
    /// treats *anything but* `[1,0,0,0]` as `None`, not just `[0,0,0,0]`;
    /// this proves that's actually what the code does.
    #[kani::proof]
    fn prove_read_coption_address_correctness() {
        let data: [u8; 36] = kani::any();
        let result = read_coption_address(&data, 0);
        if data[0..4] == [1u8, 0, 0, 0] {
            let got: Option<[u8; 32]> = result.map(|a| a.as_ref().try_into().unwrap());
            assert_eq!(got, Some(data[4..36].try_into().unwrap()));
        } else {
            assert!(
                result.is_none(),
                "any tag other than the canonical Some encoding must decode as None"
            );
        }
    }

    /// `is_canonical_coption_tag` never panics for any slice length (a
    /// `==` comparison against a fixed-size array is `false`, not a panic,
    /// for a mismatched length) and correctly recognizes exactly the two
    /// real SPL-written tag encodings.
    #[kani::proof]
    fn prove_is_canonical_coption_tag_correctness() {
        let tag: [u8; 4] = kani::any();
        let expected = tag == [0u8, 0, 0, 0] || tag == [1u8, 0, 0, 0];
        assert_eq!(is_canonical_coption_tag(&tag), expected);
    }

    /// Proves `validate_token_account_layout` never panics for any account
    /// bytes at least as long as the real SPL `Account` struct (165 bytes)
    /// — every offset it indexes (72, 108, 109, 129) is a fixed literal
    /// compared against the length-checked buffer, but this proves that
    /// holds rather than eyeballing the offsets against the doc comment.
    #[kani::proof]
    fn prove_validate_token_account_layout_never_panics() {
        let data: [u8; 165] = kani::any();
        let exact_len: bool = kani::any();
        let _ = validate_token_account_layout(&data, exact_len);
    }

    /// Same proof, but for a Token-2022 account with a TLV extension region
    /// appended past the base 165 bytes — `exact_len = false`'s whole
    /// reason to exist, per this function's own doc comment.
    #[kani::proof]
    fn prove_validate_token_account_layout_with_extensions_never_panics() {
        let data: [u8; 200] = kani::any();
        let _ = validate_token_account_layout(&data, false);
    }

    /// Same class of proof for the 82-byte `Mint` layout.
    #[kani::proof]
    fn prove_validate_mint_layout_never_panics() {
        let data: [u8; 82] = kani::any();
        let exact_len: bool = kani::any();
        let _ = validate_mint_layout(&data, exact_len);
    }

    #[kani::proof]
    fn prove_validate_mint_layout_with_extensions_never_panics() {
        let data: [u8; 120] = kani::any();
        let _ = validate_mint_layout(&data, false);
    }

    /// `TokenAccount::check_mint`/`check_authority` and
    /// `Mint::check_decimals`/`check_authority`/`check_freeze_authority`
    /// never panic for any account bytes and any expected value — each
    /// backs a real `#[account(token::mint = ...)]`-style constraint, so a
    /// panic here would abort a transaction that should have gotten a clean
    /// constraint-violation error instead.
    #[kani::proof]
    fn prove_token_account_checks_never_panic() {
        let data: [u8; 165] = kani::any();
        let expected: Address = Address::new_from_array(kani::any());
        let _ = TokenAccount::check_mint(&data, &expected);
        let _ = TokenAccount::check_authority(&data, &expected);
    }

    #[kani::proof]
    fn prove_mint_checks_never_panic() {
        let data: [u8; 82] = kani::any();
        let expected: Address = Address::new_from_array(kani::any());
        let expected_decimals: u8 = kani::any();
        let _ = Mint::check_decimals(&data, expected_decimals);
        let _ = Mint::check_authority(&data, &expected);
        let _ = Mint::check_freeze_authority(&data, &expected);
    }

    /// Correctness: `check_mint`/`check_authority` must return `Ok(())` if
    /// and only if the compared field actually equals `expected` — not an
    /// approximation that could, say, accept a prefix match.
    #[kani::proof]
    fn prove_check_mint_correctness() {
        let data: [u8; 165] = kani::any();
        let expected: Address = Address::new_from_array(kani::any());
        let result = TokenAccount::check_mint(&data, &expected);
        let actual = Address::new_from_array(data[0..32].try_into().unwrap());
        assert_eq!(result.is_ok(), actual == expected);
    }
}
