//! The SPL Token and Token-2022 knowledge `naclac-client` carries by hand —
//! program IDs, the associated-token-account `Create` instruction, and the
//! base token-account and mint layouts — checked against the real `spl-*`
//! crates, which are dev-dependencies only.

use {
    naclac_client::{
        utils::create_associated_token_account_instruction, Address, Backend, Mint,
        NaclacClientError, NaclacDecode, NaclacProvider, NaclacTransactionMetadata, RpcCluster,
        TokenAccount, ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
    },
    solana_account::Account,
    solana_hash::Hash,
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

#[derive(Default)]
struct AccountStore {
    accounts: Mutex<HashMap<Address, Account>>,
}

impl Backend for AccountStore {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, NaclacClientError> {
        Ok(self.accounts.lock().unwrap().get(address).cloned())
    }

    fn get_balance(&self, _address: &Address) -> Result<u64, NaclacClientError> {
        Ok(0)
    }

    fn latest_blockhash(&self) -> Result<Hash, NaclacClientError> {
        Ok(Hash::default())
    }

    fn minimum_balance_for_rent_exemption(
        &self,
        _data_len: usize,
    ) -> Result<u64, NaclacClientError> {
        Ok(0)
    }

    fn send_transaction(
        &self,
        _transaction: &VersionedTransaction,
        _account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        Err(NaclacClientError::General("not used".to_string()))
    }

    fn simulate_transaction(
        &self,
        _transaction: &VersionedTransaction,
        _account_names: Option<&[&str]>,
    ) -> Result<NaclacTransactionMetadata, NaclacClientError> {
        Err(NaclacClientError::General("not used".to_string()))
    }

    fn airdrop(&self, _address: &Address, _lamports: u64) -> Result<(), NaclacClientError> {
        Ok(())
    }

    fn set_account(&self, address: &Address, account: Account) -> Result<(), NaclacClientError> {
        self.accounts.lock().unwrap().insert(*address, account);
        Ok(())
    }
}

fn token_account_bytes(mint: [u8; 32], owner: [u8; 32], amount: u64, frozen: bool) -> Vec<u8> {
    use spl_token::state::{Account, AccountState};
    let account = Account {
        mint: mint.into(),
        owner: owner.into(),
        amount,
        state: if frozen {
            AccountState::Frozen
        } else {
            AccountState::Initialized
        },
        ..Default::default()
    };
    let mut data = vec![0u8; Account::LEN];
    Account::pack(account, &mut data).unwrap();
    data
}

fn mint_bytes(supply: u64, decimals: u8) -> Vec<u8> {
    use spl_token::state::Mint;
    let mint = Mint {
        supply,
        decimals,
        is_initialized: true,
        ..Default::default()
    };
    let mut data = vec![0u8; Mint::LEN];
    Mint::pack(mint, &mut data).unwrap();
    data
}

/// Data of the length a Token-2022 account or mint with extensions has: the
/// base layout, zero padding, and the account-type byte at index 165.
fn extended(mut base: Vec<u8>, account_type: u8) -> Vec<u8> {
    base.resize(170, 0);
    base[165] = account_type;
    base
}

#[test]
fn program_ids_match_the_spl_crates() {
    assert_eq!(TOKEN_PROGRAM_ID.to_bytes(), spl_token::id().to_bytes());
    assert_eq!(
        TOKEN_2022_PROGRAM_ID.to_bytes(),
        spl_token_2022::id().to_bytes()
    );
    assert_eq!(
        ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes(),
        spl_associated_token_account::id().to_bytes()
    );
}

#[test]
fn the_associated_token_account_instruction_matches_the_spl_builder() {
    for token_program in [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
        let (funding, owner, mint) = (
            Address::new_unique(),
            Address::new_unique(),
            Address::new_unique(),
        );
        let ours =
            create_associated_token_account_instruction(&funding, &owner, &mint, &token_program);
        let theirs = spl_associated_token_account::instruction::create_associated_token_account(
            &funding.to_bytes().into(),
            &owner.to_bytes().into(),
            &mint.to_bytes().into(),
            &token_program.to_bytes().into(),
        );

        assert_eq!(ours.program_id.to_bytes(), theirs.program_id.to_bytes());
        assert_eq!(ours.data, theirs.data);
        assert_eq!(ours.accounts.len(), theirs.accounts.len());
        for (ours, theirs) in ours.accounts.iter().zip(&theirs.accounts) {
            assert_eq!(ours.pubkey.to_bytes(), theirs.pubkey.to_bytes());
            assert_eq!(ours.is_signer, theirs.is_signer);
            assert_eq!(ours.is_writable, theirs.is_writable);
        }
        assert_eq!(ours.accounts[4].pubkey, SYSTEM_PROGRAM_ID);
    }
}

#[test]
fn the_base_layouts_read_the_same_fields_as_the_spl_unpackers() {
    let (mint, owner) = ([3u8; 32], [4u8; 32]);

    for frozen in [false, true] {
        let data = token_account_bytes(mint, owner, 1_234_567, frozen);
        let spl = spl_token::state::Account::unpack(&data).unwrap();
        let ours = TokenAccount::naclac_decode(&data).unwrap();
        assert_eq!(ours.mint().to_bytes(), spl.mint.to_bytes());
        assert_eq!(ours.owner().to_bytes(), spl.owner.to_bytes());
        assert_eq!(ours.amount(), spl.amount);
        assert!(ours.is_initialized());
    }

    let data = mint_bytes(9_000, 6);
    let spl = spl_token::state::Mint::unpack(&data).unwrap();
    let ours = Mint::naclac_decode(&data).unwrap();
    assert_eq!(ours.supply(), spl.supply);
    assert_eq!(ours.decimals(), spl.decimals);
    assert!(ours.is_initialized());
}

#[test]
fn uninitialized_and_invalid_state_bytes_are_not_initialized() {
    let mut data = token_account_bytes([1; 32], [2; 32], 5, false);
    data[108] = 0;
    assert!(!TokenAccount::naclac_decode(&data).unwrap().is_initialized());
    assert!(spl_token::state::Account::unpack(&data).is_err());
    data[108] = 3;
    assert!(!TokenAccount::naclac_decode(&data).unwrap().is_initialized());
    assert!(spl_token::state::Account::unpack(&data).is_err());

    let mut data = mint_bytes(1, 2);
    data[45] = 0;
    assert!(!Mint::naclac_decode(&data).unwrap().is_initialized());
    data[45] = 2;
    assert!(!Mint::naclac_decode(&data).unwrap().is_initialized());
    assert!(spl_token::state::Mint::unpack(&data).is_err());
}

#[test]
fn token_2022_accounts_with_extensions_are_read_where_the_spl_unpacker_fails() {
    let account = extended(token_account_bytes([5; 32], [6; 32], 777, false), 2);
    assert!(spl_token_2022::state::Account::unpack(&account).is_err());
    let ours = TokenAccount::naclac_decode(&account).unwrap();
    assert_eq!(ours.amount(), 777);
    assert!(ours.is_initialized());

    let mint = extended(mint_bytes(40, 9), 1);
    assert!(spl_token_2022::state::Mint::unpack(&mint).is_err());
    let ours = Mint::naclac_decode(&mint).unwrap();
    assert_eq!(ours.decimals(), 9);
    assert!(ours.is_initialized());
}

fn provider_with(accounts: &[(Address, Address, Vec<u8>)]) -> NaclacProvider {
    let store = Arc::new(AccountStore::default());
    for (address, owner, data) in accounts {
        store
            .set_account(
                address,
                Account {
                    lamports: 1,
                    data: data.clone(),
                    owner: *owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
    }
    NaclacProvider::with_backend(store, Keypair::new(), RpcCluster::Custom)
}

#[test]
fn get_token_balance_reads_token_and_token_2022_accounts() {
    let (token_account, mint) = (Address::new_unique(), Address::new_unique());
    let owner = [8u8; 32];

    let cases = [
        (
            TOKEN_PROGRAM_ID,
            token_account_bytes(mint.to_bytes(), owner, 500, false),
            mint_bytes(1_000, 6),
        ),
        (
            TOKEN_2022_PROGRAM_ID,
            token_account_bytes(mint.to_bytes(), owner, 600, true),
            mint_bytes(1_000, 8),
        ),
        (
            TOKEN_2022_PROGRAM_ID,
            extended(token_account_bytes(mint.to_bytes(), owner, 700, false), 2),
            extended(mint_bytes(1_000, 9), 1),
        ),
    ];
    for (program, account_data, mint_data) in cases {
        let expected_amount = u64::from_le_bytes(account_data[64..72].try_into().unwrap());
        let expected_decimals = mint_data[44];
        let provider = provider_with(&[
            (token_account, program, account_data),
            (mint, program, mint_data),
        ]);
        assert_eq!(
            provider.get_token_balance(&token_account).unwrap(),
            (expected_amount, expected_decimals)
        );
    }
}

#[test]
fn get_token_balance_rejects_foreign_and_uninitialized_accounts() {
    let (token_account, mint) = (Address::new_unique(), Address::new_unique());
    let account_data = token_account_bytes(mint.to_bytes(), [8; 32], 5, false);
    let mint_data = mint_bytes(1, 2);

    let foreign = provider_with(&[
        (token_account, Address::new_unique(), account_data.clone()),
        (mint, TOKEN_PROGRAM_ID, mint_data.clone()),
    ]);
    assert!(foreign.get_token_balance(&token_account).is_err());

    let mut uninitialized_account = account_data.clone();
    uninitialized_account[108] = 0;
    let provider = provider_with(&[
        (token_account, TOKEN_PROGRAM_ID, uninitialized_account),
        (mint, TOKEN_PROGRAM_ID, mint_data.clone()),
    ]);
    assert!(provider.get_token_balance(&token_account).is_err());

    let mut uninitialized_mint = mint_data;
    uninitialized_mint[45] = 0;
    let provider = provider_with(&[
        (token_account, TOKEN_PROGRAM_ID, account_data),
        (mint, TOKEN_PROGRAM_ID, uninitialized_mint),
    ]);
    assert!(provider.get_token_balance(&token_account).is_err());
}
