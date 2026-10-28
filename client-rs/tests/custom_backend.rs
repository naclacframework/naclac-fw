//! A `Backend` implemented outside `naclac-client`, with neither the `litesvm`
//! nor the `rpc` feature required, drives a `NaclacProvider` and an
//! `InstructionBuilder` end to end. It pins the contract another SVM adapter
//! relies on: required operations route to the backend, optional ones fall back
//! to a clear "not supported" error, and a backend whose feature is not
//! compiled in is reported by name.

use {
    naclac_client::{
        AccountMeta, Backend, InstructionBuilder, NaclacClientError, NaclacProvider,
        NaclacTransactionMetadata, RpcCluster, SignerAddressExt, SYSTEM_PROGRAM_ID,
    },
    solana_account::Account,
    solana_address::Address,
    solana_hash::Hash,
    solana_keypair::Keypair,
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

#[derive(Default)]
struct MemoryBackend {
    accounts: Mutex<HashMap<Address, Account>>,
    sent: Mutex<Vec<VersionedTransaction>>,
}

fn metadata(transaction: &VersionedTransaction) -> NaclacTransactionMetadata {
    NaclacTransactionMetadata {
        signature: transaction.signatures[0],
        logs: vec!["memory backend executed".to_string()],
        compute_units_consumed: 7,
        return_data: Vec::new(),
        inner_instructions: Some(Vec::new()),
    }
}

impl Backend for MemoryBackend {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, NaclacClientError> {
        Ok(self.accounts.lock().unwrap().get(address).cloned())
    }

    fn get_balance(&self, address: &Address) -> Result<u64, NaclacClientError> {
        Ok(self
            .accounts
            .lock()
            .unwrap()
            .get(address)
            .map_or(0, |account| account.lamports))
    }

    fn latest_blockhash(&self) -> Result<Hash, NaclacClientError> {
        Ok(Hash::new_from_array([7; 32]))
    }

    fn minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> Result<u64, NaclacClientError> {
        Ok(data_len as u64 * 10)
    }

    fn send_transaction(
        &self,
        transaction: &VersionedTransaction,
        _account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        self.sent.lock().unwrap().push(transaction.clone());
        Ok(metadata(transaction))
    }

    fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
        _account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        Ok(metadata(transaction))
    }

    fn airdrop(&self, address: &Address, lamports: u64) -> Result<(), NaclacClientError> {
        self.accounts
            .lock()
            .unwrap()
            .entry(*address)
            .or_default()
            .lamports += lamports;
        Ok(())
    }

    fn set_account(&self, address: &Address, account: Account) -> Result<(), NaclacClientError> {
        self.accounts.lock().unwrap().insert(*address, account);
        Ok(())
    }
}

fn provider() -> (Arc<MemoryBackend>, NaclacProvider) {
    let backend = Arc::new(MemoryBackend::default());
    let provider =
        NaclacProvider::with_backend(backend.clone(), Keypair::new(), RpcCluster::Custom);
    (backend, provider)
}

#[test]
fn reads_and_writes_go_through_the_backend() {
    let (_backend, provider) = provider();
    let address = Address::new_unique();
    let owner = Address::new_unique();

    provider.airdrop(&address, 5).unwrap();
    assert_eq!(provider.get_balance(&address).unwrap(), 5);

    provider
        .set_account(&address, vec![1, 2, 3], &owner, 9)
        .unwrap();
    assert_eq!(provider.get_account_data(&address).unwrap(), vec![1, 2, 3]);
    assert_eq!(provider.get_account(&address).unwrap().owner, owner);

    provider.set_account_lamports(&address, 42).unwrap();
    assert_eq!(provider.get_balance(&address).unwrap(), 42);

    assert_eq!(
        provider.get_minimum_balance_for_rent_exemption(4).unwrap(),
        40
    );
    assert_eq!(
        provider.get_latest_blockhash().unwrap(),
        Hash::new_from_array([7; 32])
    );
    assert!(matches!(
        provider.get_account(&Address::new_unique()),
        Err(NaclacClientError::AccountNotFound(_))
    ));
}

#[test]
fn the_builder_sends_and_simulates_through_the_backend() {
    let (backend, provider) = provider();
    let recipient = Address::new_unique();
    let mut data = vec![2, 0, 0, 0];
    data.extend_from_slice(&1_000u64.to_le_bytes());

    let builder = InstructionBuilder::new(&provider, SYSTEM_PROGRAM_ID, data)
        .account(AccountMeta::new(provider.payer.address(), true), "from")
        .account(AccountMeta::new(recipient, false), "to");

    let simulated = builder.simulate().unwrap();
    assert_eq!(simulated.logs, ["memory backend executed"]);
    assert!(backend.sent.lock().unwrap().is_empty());

    let sent = builder.send_and_confirm().unwrap();
    assert_eq!(sent.compute_units_consumed, 7);
    let recorded = backend.sent.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].signatures[0], sent.signature);
}

#[test]
fn optional_operations_report_that_the_backend_does_not_support_them() {
    let (_backend, provider) = provider();
    let unsupported = |result: Result<(), NaclacClientError>, operation: &str| match result {
        Err(NaclacClientError::General(message)) => {
            assert_eq!(
                message,
                format!("{operation} is not supported by this backend")
            );
        }
        other => panic!("{operation}: expected the unsupported error, got {other:?}"),
    };

    unsupported(provider.expire_blockhash(), "expire_blockhash");
    unsupported(
        provider.set_clock_unix_timestamp(1),
        "set_clock_unix_timestamp",
    );
    unsupported(
        provider
            .get_program_accounts(&Address::new_unique(), None)
            .map(|_| ()),
        "get_program_accounts",
    );
    unsupported(
        provider
            .get_transaction(&solana_signature::Signature::default())
            .map(|_| ()),
        "get_transaction",
    );
}

#[test]
fn a_string_that_is_neither_a_cluster_nor_an_http_url_is_rejected_by_name() {
    for candidate in [
        "rpc",
        "Devnet",
        "127.0.0.1:8899",
        "localhost:8899",
        "",
        "http://",
        "https://",
        "ftp://example.com",
    ] {
        assert!(
            matches!(
                NaclacProvider::new(candidate, Keypair::new()),
                Err(NaclacClientError::General(message))
                    if message.contains("is not a known cluster or an http(s) URL")
            ),
            "{candidate:?} should be rejected as neither a cluster name nor an http(s) URL"
        );
    }
}

#[cfg(feature = "rpc")]
#[test]
fn a_custom_http_url_is_accepted_and_classified_by_its_host() {
    let custom = NaclacProvider::new("http://10.0.0.5:8899", Keypair::new()).unwrap();
    assert_eq!(custom.cluster, RpcCluster::Custom);
    let local = NaclacProvider::new("http://localhost:9000", Keypair::new()).unwrap();
    assert_eq!(local.cluster, RpcCluster::Localnet);
    assert!(matches!(
        NaclacProvider::new_rpc("127.0.0.1:8899", Keypair::new()),
        Err(NaclacClientError::General(message)) if message.contains("needs its scheme")
    ));
}

#[cfg(not(feature = "litesvm"))]
#[test]
fn the_litesvm_constructor_names_the_missing_feature() {
    assert!(matches!(
        NaclacProvider::new("litesvm", Keypair::new()),
        Err(NaclacClientError::General(message)) if message.contains("`litesvm` feature")
    ));
}

#[cfg(not(feature = "rpc"))]
#[test]
fn the_rpc_constructors_name_the_missing_feature() {
    for cluster in ["devnet", "localnet", "http://127.0.0.1:8899"] {
        assert!(matches!(
            NaclacProvider::new(cluster, Keypair::new()),
            Err(NaclacClientError::General(message)) if message.contains("`rpc` feature")
        ));
    }
}
