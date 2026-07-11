use crate::error::NaclacClientError;
use crate::provider::{ClientBackend, NaclacProvider};
use borsh::BorshDeserialize;
use bytemuck::Pod;
use solana_account::Account;
use solana_address::Address;
use solana_program::pubkey::Pubkey;
use solana_rpc_client_api::config::RpcProgramAccountsConfig;
use solana_rpc_client_api::filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};

/// A type that knows how to decode itself from raw on-chain account bytes.
///
/// Implemented once per type, with the correct decode strategy (Borsh vs raw
/// `Pod` byte-cast) baked in at THAT type's own compile time — for generated
/// component types, via `#[cfg(feature = "borsh")]` on the generated client
/// crate (itself set correctly from the target program's real IDL at
/// generation time). This is what lets [`AccountFetcher::fetch`] be a single,
/// always-correct method name for any `T`: the Borsh-vs-zero-copy ambiguity is
/// resolved at compile time, on the type itself, never guessed at runtime from
/// the raw bytes. Runtime guessing isn't just less elegant — it's actively
/// unsafe here: a `Pod` byte-cast essentially never fails just because the
/// bytes are "wrong" (any bit pattern is a valid `Pod` value), so a wrong
/// guess would silently produce garbage field values instead of a clean error.
pub trait NaclacDecode: Sized {
    /// This type's 8-byte discriminator, or `None` for foreign types with no
    /// Naclac discriminator prefix (e.g. raw SPL `TokenAccount`/`Mint`).
    const DISCRIMINATOR: Option<[u8; 8]> = None;

    fn naclac_decode(data: &[u8]) -> Result<Self, NaclacClientError>;
}

/// Checks the 8-byte discriminator prefix matches, then Borsh-deserializes the rest.
pub fn decode_borsh_checked<T: BorshDeserialize>(
    data: &[u8],
    discriminator: [u8; 8],
) -> Result<T, NaclacClientError> {
    if data.len() < 8 || data[..8] != discriminator {
        return Err(NaclacClientError::DeserializationError(
            "Account discriminator mismatch (wrong account type, or account not yet initialized)"
                .to_string(),
        ));
    }
    let mut reader = &data[8..];
    T::deserialize(&mut reader).map_err(|e| {
        NaclacClientError::DeserializationError(format!("Borsh decode failed: {:?}", e))
    })
}

/// Checks the 8-byte discriminator prefix matches, then byte-casts the rest as `T`.
pub fn decode_pod_checked<T: Pod>(
    data: &[u8],
    discriminator: [u8; 8],
) -> Result<T, NaclacClientError> {
    let size = std::mem::size_of::<T>();
    if data.len() < 8 + size || data[..8] != discriminator {
        return Err(NaclacClientError::DeserializationError(
            "Account discriminator mismatch (wrong account type, or account not yet initialized)"
                .to_string(),
        ));
    }
    let casted = bytemuck::try_from_bytes::<T>(&data[8..8 + size]).map_err(|e| {
        NaclacClientError::DeserializationError(format!("Zero-Copy cast failed: {:?}", e))
    })?;
    Ok(*casted)
}

/// Byte-casts `data` as `T` with no discriminator prefix at all — for foreign
/// account types (e.g. SPL `TokenAccount`/`Mint`) that carry no Naclac discriminator.
pub fn decode_pod_unchecked<T: Pod>(data: &[u8]) -> Result<T, NaclacClientError> {
    let size = std::mem::size_of::<T>();
    if data.len() < size {
        return Err(NaclacClientError::DeserializationError(format!(
            "Account data length ({}) is less than required size ({})",
            data.len(),
            size
        )));
    }
    let casted = bytemuck::try_from_bytes::<T>(&data[..size]).map_err(|e| {
        NaclacClientError::DeserializationError(format!("Zero-Copy cast failed: {:?}", e))
    })?;
    Ok(*casted)
}

pub struct AccountFetcher<'a> {
    pub provider: &'a NaclacProvider,
}

impl<'a> AccountFetcher<'a> {
    pub fn new(provider: &'a NaclacProvider) -> Self {
        Self { provider }
    }

    /// Fetches and decodes a single account via its own [`NaclacDecode`] impl.
    ///
    /// The single, recommended way to fetch any account: correct regardless
    /// of whether the target program is Borsh or zero-copy mode, since the
    /// decode strategy is baked into `T` at its own compile time rather than
    /// guessed here. For a foreign type with no `NaclacDecode` impl yet, write
    /// one — see `TokenAccount`/`Mint` in `token.rs` for a minimal example.
    pub fn fetch<T: NaclacDecode>(&self, address: &Address) -> Result<T, NaclacClientError> {
        let account = self.provider.get_account(address)?;
        T::naclac_decode(&account.data)
    }

    /// Fetches and decodes multiple accounts via their own [`NaclacDecode`] impl.
    pub fn fetch_multiple<T: NaclacDecode>(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Option<T>>, NaclacClientError> {
        let accounts = self.get_multiple_accounts(addresses)?;
        Ok(accounts
            .into_iter()
            .map(|acc_opt| acc_opt.and_then(|acc| T::naclac_decode(&acc.data).ok()))
            .collect())
    }

    /// Like `fetch_multiple`, but returns a map keyed by address instead of a
    /// parallel `Vec`.
    pub fn fetch_multiple_as_map<T: NaclacDecode>(
        &self,
        addresses: &[Address],
    ) -> Result<std::collections::HashMap<Address, Option<T>>, NaclacClientError> {
        let results = self.fetch_multiple::<T>(addresses)?;
        Ok(addresses.iter().cloned().zip(results).collect())
    }

    /// Fetches and decodes every on-chain `T` account owned by `program_id`,
    /// via its own [`NaclacDecode`] impl. Requires `T::DISCRIMINATOR` to be
    /// `Some(..)` — there's no way to filter which accounts belong to `T`
    /// without one, so this returns an error for discriminator-less types.
    pub fn fetch_all<T: NaclacDecode>(
        &self,
        program_id: &Address,
    ) -> Result<Vec<(Address, T)>, NaclacClientError> {
        let discriminator = T::DISCRIMINATOR.ok_or_else(|| {
            NaclacClientError::General(
                "fetch_all::<T> requires T::DISCRIMINATOR to be set (discriminator-less types \
                 have no way to be filtered by program_id alone)"
                    .to_string(),
            )
        })?;
        let accounts = self.get_program_accounts(program_id, discriminator)?;
        Ok(accounts
            .into_iter()
            .filter_map(|(pk, acc)| T::naclac_decode(&acc.data).ok().map(|v| (pk, v)))
            .collect())
    }

    fn get_multiple_accounts(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Option<Account>>, NaclacClientError> {
        match &self.provider.backend {
            ClientBackend::LiteSVM(_) => {
                let mut results = Vec::with_capacity(addresses.len());
                for addr in addresses {
                    match self.provider.get_account(addr) {
                        Ok(acc) => results.push(Some(acc)),
                        Err(_) => results.push(None),
                    }
                }
                Ok(results)
            }
            ClientBackend::Rpc(client) => {
                let pubkeys: Vec<Pubkey> = addresses
                    .iter()
                    .map(|addr| Pubkey::new_from_array(addr.to_bytes()))
                    .collect();
                client
                    .get_multiple_accounts_with_commitment(&pubkeys, self.provider.commitment)
                    .map_err(|e| {
                        NaclacClientError::RpcError(format!("get_multiple_accounts failed: {}", e))
                    })
                    .map(|res| res.value)
            }
        }
    }

    fn get_program_accounts(
        &self,
        program_id: &Address,
        discriminator: [u8; 8],
    ) -> Result<Vec<(Address, Account)>, NaclacClientError> {
        match &self.provider.backend {
            ClientBackend::LiteSVM(_) => {
                Err(NaclacClientError::General("get_program_accounts is not supported by the LiteSVM backend. Please track account addresses manually during tests.".to_string()))
            }
            ClientBackend::Rpc(client) => {
                let mut config = RpcProgramAccountsConfig::default();
                config.account_config.commitment = Some(self.provider.commitment);
                config.account_config.encoding = Some(solana_account_decoder::UiAccountEncoding::Base64);

                let encoded_discriminator = bs58::encode(discriminator).into_string();
                let memcmp = Memcmp::new(0, MemcmpEncodedBytes::Base58(encoded_discriminator));
                config.filters = Some(vec![RpcFilterType::Memcmp(memcmp)]);

                let program_pubkey = Pubkey::new_from_array(program_id.to_bytes());
                let accounts = client.get_program_ui_accounts_with_config(&program_pubkey, config)
                    .map_err(|e| NaclacClientError::RpcError(format!("get_program_accounts failed: {}", e)))?;

                let mapped = accounts.into_iter()
                    .filter_map(|(pk, ui_acc)| {
                        ui_acc.decode().map(|acc| (Address::new_from_array(pk.to_bytes()), acc))
                    })
                    .collect();
                Ok(mapped)
            }
        }
    }
}
