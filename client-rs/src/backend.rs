//! The extension point for execution backends other than the built-in
//! LiteSVM (`litesvm` feature) and RPC (`rpc` feature) ones. A type that
//! implements [`Backend`] is handed to
//! [`NaclacProvider::with_backend`](crate::provider::NaclacProvider::with_backend)
//! and reached through `ClientBackend::Custom`.

use {
    crate::{error::NaclacClientError, provider::NaclacTransactionMetadata},
    solana_account::Account,
    solana_address::Address,
    solana_hash::Hash,
    solana_signature::Signature,
    solana_transaction::versioned::VersionedTransaction,
};

/// The error an optional [`Backend`] operation returns when the backend has
/// no equivalent for it.
pub fn unsupported(operation: &str) -> NaclacClientError {
    NaclacClientError::General(format!("{operation} is not supported by this backend"))
}

/// An account store plus a way to execute transactions against it.
///
/// The first group of methods is required of every backend. The second group
/// is optional: each default body returns [`unsupported`], and the provider
/// surfaces that error unchanged. `Send + Sync` is required because a
/// `NaclacProvider` is cloned and shared across test threads; a backend whose
/// engine is not thread-safe must confine it to a thread of its own.
pub trait Backend: Send + Sync {
    /// `None` when the account does not exist.
    fn get_account(&self, address: &Address) -> Result<Option<Account>, NaclacClientError>;

    /// `0` for an account that does not exist.
    fn get_balance(&self, address: &Address) -> Result<u64, NaclacClientError>;

    fn latest_blockhash(&self) -> Result<Hash, NaclacClientError>;

    fn minimum_balance_for_rent_exemption(&self, data_len: usize)
        -> Result<u64, NaclacClientError>;

    /// Executes `transaction` and commits its effects. A failed transaction
    /// is returned as `NaclacClientError::TransactionFailed`, built with
    /// [`translate_transaction_failure`](crate::provider::translate_transaction_failure)
    /// so program error codes are translated to their names via `account_names`.
    fn send_transaction(
        &self,
        transaction: &VersionedTransaction,
        account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError>;

    /// Executes `transaction` without committing any of its effects.
    fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
        account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError>;

    fn airdrop(&self, address: &Address, lamports: u64) -> Result<(), NaclacClientError>;

    fn get_multiple_accounts(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Option<Account>>, NaclacClientError> {
        addresses
            .iter()
            .map(|address| self.get_account(address))
            .collect()
    }

    fn get_transaction(
        &self,
        _signature: &Signature,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        Err(unsupported("get_transaction"))
    }

    /// `(address, data)` of every account owned by `program_id` whose data
    /// starts with `discriminator`, when one is given.
    fn get_program_accounts(
        &self,
        _program_id: &Address,
        _discriminator: Option<&[u8]>,
    ) -> Result<Vec<(Address, Vec<u8>)>, NaclacClientError> {
        Err(unsupported("get_program_accounts"))
    }

    fn expire_blockhash(&self) -> Result<(), NaclacClientError> {
        Err(unsupported("expire_blockhash"))
    }

    /// Overwrites the whole account at `address`.
    fn set_account(&self, _address: &Address, _account: Account) -> Result<(), NaclacClientError> {
        Err(unsupported("set_account"))
    }

    fn set_clock_unix_timestamp(&self, _unix_timestamp: i64) -> Result<(), NaclacClientError> {
        Err(unsupported("set_clock_unix_timestamp"))
    }

    /// Loads the program binary `elf` at `program_id`.
    fn add_program(&self, _program_id: &Address, _elf: &[u8]) -> Result<(), NaclacClientError> {
        Err(unsupported("add_program"))
    }
}
