#[cfg(feature = "borsh")]
use crate::sdk_core::borsh::{BorshDeserialize, BorshSerialize};
#[cfg_attr(feature = "borsh", derive(Clone, Debug, BorshSerialize, BorshDeserialize))]
#[cfg_attr(feature = "borsh", borsh(crate = "crate::sdk_core::borsh"))]
#[cfg_attr(not(feature = "borsh"), derive(Copy, Clone, Debug))]
#[cfg_attr(not(feature = "borsh"), repr(C))]
pub struct EscrowState {
    pub maker: crate::sdk_core::Address,
    pub mint_a: crate::sdk_core::Address,
    pub mint_b: crate::sdk_core::Address,
    pub amount_a: u64,
    pub amount_b: u64,
    pub bump: u8,
}
/// 8-byte on-chain discriminator for `EscrowState` accounts.
pub const ESCROWSTATE_DISCRIMINATOR: [u8; 8] = [19, 90, 148, 111, 55, 130, 229, 108];
#[cfg(not(feature = "borsh"))]
unsafe impl crate::sdk_core::bytemuck::Zeroable for EscrowState {}
#[cfg(not(feature = "borsh"))]
unsafe impl crate::sdk_core::bytemuck::Pod for EscrowState {}
#[cfg(feature = "offchain")]
#[cfg(not(feature = "borsh"))]
/// Borrows account data as `EscrowState` after skipping the 8-byte discriminator.
/// The returned reference is zero-copy: no allocation, no deserialization.
pub fn fetch_escrow_state(data: &[u8]) -> &EscrowState {
    naclac_client::bytemuck::from_bytes::<EscrowState>(&data[8..])
}
#[cfg(feature = "offchain")]
#[cfg(not(feature = "borsh"))]
/// Borrows account data mutably as `EscrowState` (zero-copy write access).
pub fn fetch_escrow_state_mut(data: &mut [u8]) -> &mut EscrowState {
    naclac_client::bytemuck::from_bytes_mut::<EscrowState>(&mut data[8..])
}
#[cfg(feature = "offchain")]
#[cfg(not(feature = "borsh"))]
/// Fetches ALL on-chain `EscrowState` accounts owned by `program_id` (Zero-Copy).
pub fn fetch_all_escrow_state(
    provider: &naclac_client::NaclacProvider,
    program_id: &naclac_client::Address,
) -> Result<
    crate::sdk_core::Vec<(naclac_client::Address, EscrowState)>,
    naclac_client::NaclacClientError,
> {
    let raw_accounts = provider
        .get_program_accounts(program_id, Some(&ESCROWSTATE_DISCRIMINATOR))?;
    let mut results = crate::sdk_core::Vec::new();
    for (address, data) in raw_accounts {
        if data.len() < 8 {
            continue;
        }
        let value = fetch_escrow_state(&data);
        results.push((address, *value));
    }
    Ok(results)
}
#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
/// Fetches and deserializes a `EscrowState` account from the chain (Borsh).
pub fn fetch_escrow_state(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<EscrowState, naclac_client::NaclacClientError> {
    let raw = provider.get_account_data(address)?;
    if raw.len() < 8 || raw[..8] != ESCROWSTATE_DISCRIMINATOR {
        return Err(
            naclac_client::NaclacClientError::General(
                format!("Invalid discriminator for account {}", address),
            ),
        );
    }
    naclac_client::borsh::from_slice::<EscrowState>(&raw[8..])
        .map_err(|e| naclac_client::NaclacClientError::DeserializationError(
            e.to_string(),
        ))
}
#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
/// Fetches a `EscrowState` account (Borsh). Returns `None` if the account does not exist.
pub fn fetch_maybe_escrow_state(
    provider: &naclac_client::NaclacProvider,
    address: &naclac_client::Address,
) -> Result<Option<EscrowState>, naclac_client::NaclacClientError> {
    match fetch_escrow_state(provider, address) {
        Ok(a) => Ok(Some(a)),
        Err(naclac_client::NaclacClientError::AccountNotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
#[cfg(feature = "offchain")]
#[cfg(feature = "borsh")]
/// Fetches ALL on-chain `EscrowState` accounts owned by `program_id` (Borsh).
pub fn fetch_all_escrow_state(
    provider: &naclac_client::NaclacProvider,
    program_id: &naclac_client::Address,
) -> Result<
    crate::sdk_core::Vec<(naclac_client::Address, EscrowState)>,
    naclac_client::NaclacClientError,
> {
    let raw_accounts = provider
        .get_program_accounts(program_id, Some(&ESCROWSTATE_DISCRIMINATOR))?;
    let mut results = crate::sdk_core::Vec::new();
    for (address, data) in raw_accounts {
        if data.len() < 8 {
            continue;
        }
        match naclac_client::borsh::from_slice::<EscrowState>(&data[8..]) {
            Ok(account) => results.push((address, account)),
            Err(_) => continue,
        }
    }
    Ok(results)
}
