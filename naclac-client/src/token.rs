use crate::error::NaclacClientError;
use crate::fetcher::{decode_pod_unchecked, NaclacDecode};
use solana_address::Address;

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct TokenAccount(pub [u8; 165]);

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Mint(pub [u8; 82]);

// Raw SPL types — no Naclac 8-byte discriminator prefix, and always the same
// fixed byte layout regardless of any particular Naclac program's own Borsh
// vs zero-copy mode (their layout is owned by the SPL Token program, not us).
impl NaclacDecode for TokenAccount {
    const DISCRIMINATOR: Option<[u8; 8]> = None;
    fn naclac_decode(data: &[u8]) -> Result<Self, NaclacClientError> {
        decode_pod_unchecked::<Self>(data)
    }
}

impl NaclacDecode for Mint {
    const DISCRIMINATOR: Option<[u8; 8]> = None;
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
    }

    #[kani::proof]
    fn prove_mint_accessors_never_panic() {
        let mint = Mint(kani::any());
        let _ = mint.supply();
        let _ = mint.decimals();
    }
}
