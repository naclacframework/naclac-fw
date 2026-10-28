use {
    crate::{
        error::NaclacClientError,
        fetcher::{decode_pod_unchecked, NaclacDecode},
    },
    solana_address::Address,
};

const TOKEN_ACCOUNT_LEN: usize = 165;
const MINT_LEN: usize = 82;

/// The fixed base layout shared by SPL Token and Token-2022 token accounts.
/// A Token-2022 account carrying extensions is longer than this; the base
/// layout is its unchanged prefix.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct TokenAccount(pub [u8; TOKEN_ACCOUNT_LEN]);

/// The fixed base layout shared by SPL Token and Token-2022 mints. A
/// Token-2022 mint carrying extensions is longer than this; the base layout
/// is its unchanged prefix.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Mint(pub [u8; MINT_LEN]);

// Raw SPL types — no Naclac discriminator prefix, and always the same
// fixed byte layout regardless of any particular Naclac program's own Borsh
// vs zero-copy mode (their layout is owned by the SPL Token program, not us).
impl NaclacDecode for TokenAccount {
    const DISCRIMINATOR: Option<&'static [u8]> = None;
    fn naclac_decode(data: &[u8]) -> Result<Self, NaclacClientError> {
        decode_pod_unchecked::<Self>(data)
    }
}

impl NaclacDecode for Mint {
    const DISCRIMINATOR: Option<&'static [u8]> = None;
    fn naclac_decode(data: &[u8]) -> Result<Self, NaclacClientError> {
        decode_pod_unchecked::<Self>(data)
    }
}

// Bytemuck safety implementations for Zero-Copy deserialization
unsafe impl bytemuck::Pod for TokenAccount {}
unsafe impl bytemuck::Zeroable for TokenAccount {}

unsafe impl bytemuck::Pod for Mint {}
unsafe impl bytemuck::Zeroable for Mint {}

impl TokenAccount {
    pub const LEN: usize = TOKEN_ACCOUNT_LEN;

    /// `false` for an uninitialized account and for a state byte that is not
    /// `Uninitialized` (0), `Initialized` (1) or `Frozen` (2).
    pub fn is_initialized(&self) -> bool {
        matches!(self.0[108], 1 | 2)
    }

    pub fn mint(&self) -> Address {
        Address::new_from_array(self.0[0..32].try_into().unwrap())
    }

    pub fn owner(&self) -> Address {
        Address::new_from_array(self.0[32..64].try_into().unwrap())
    }

    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.0[64..72].try_into().unwrap())
    }
}

impl Mint {
    pub const LEN: usize = MINT_LEN;

    /// `false` for an uninitialized mint and for an `is_initialized` byte
    /// that is neither 0 nor 1.
    pub fn is_initialized(&self) -> bool {
        self.0[45] == 1
    }

    pub fn supply(&self) -> u64 {
        u64::from_le_bytes(self.0[36..44].try_into().unwrap())
    }

    pub fn decimals(&self) -> u8 {
        self.0[44]
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves every `TokenAccount`/`Mint` accessor never panics for any
    /// raw byte pattern — since every offset is a fixed literal against a
    /// fixed-size array (`[u8; 165]`/`[u8; 82]`), this is a trivial but
    /// genuinely exhaustive sanity proof: any `Pod` byte pattern is a valid
    /// input via `kani::any()`, unlike account-derived offsets elsewhere in
    /// this workspace.
    #[kani::proof]
    fn prove_token_account_accessors_never_panic() {
        let account = TokenAccount(kani::any());
        let _ = account.mint();
        let _ = account.owner();
        let _ = account.amount();
        let _ = account.is_initialized();
    }

    #[kani::proof]
    fn prove_mint_accessors_never_panic() {
        let mint = Mint(kani::any());
        let _ = mint.supply();
        let _ = mint.decimals();
        let _ = mint.is_initialized();
    }
}
