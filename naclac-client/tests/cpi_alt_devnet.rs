//! Devnet check that a self-CPI event is decoded when the transaction reaches
//! its `event_authority` account through an address lookup table, so the
//! inner instruction's account indices point into the loaded-address section
//! rather than the static keys. Uses the `events` test program deployed on
//! devnet at `PROGRAM_ID` (`tests/events`); needs network access and a
//! devnet-funded wallet.

use bytemuck::{Pod, Zeroable};
use naclac_client::{load_node_wallet, NaclacEvent, NaclacProvider};
use solana_address::Address;
use solana_address_lookup_table_interface::{
    instruction::{create_lookup_table, extend_lookup_table},
    state::AddressLookupTable,
};
use solana_commitment_config::CommitmentConfig;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::{v0, AddressLookupTableAccount, VersionedMessage};
use solana_rpc_client::rpc_client::RpcClient;
use solana_signer::Signer;
use solana_transaction::{versioned::VersionedTransaction, Transaction};

const PROGRAM_ID: Address = Address::from_str_const("7LLnnxsXFgpgmx4S8VqjpBrBmpgawekjiSfX1KsCs2AL");
const EVENT_CPI_EMIT_DISCRIMINATOR: [u8; 8] = [115, 58, 67, 15, 26, 157, 167, 201];
const COUNTER_INCREMENTED_DISCRIMINATOR: [u8; 8] = [219, 181, 183, 220, 88, 58, 114, 198];
const EVENT_DISPATCH_TAG: [u8; 8] = [34, 131, 189, 8, 189, 141, 224, 48];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct CounterIncremented {
    new_count: u64,
}

impl NaclacEvent for CounterIncremented {
    const DISCRIMINATOR: &'static [u8] = &COUNTER_INCREMENTED_DISCRIMINATOR;
    const PROGRAM_ID: Address = PROGRAM_ID;
    const CPI_TAG: &'static [u8] = &EVENT_DISPATCH_TAG;
}

#[test]
#[ignore = "needs network access, a funded devnet wallet, and the events program deployed on devnet"]
fn self_cpi_event_decodes_when_the_authority_comes_from_a_lookup_table() {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let payer_address = payer.pubkey();
    let client = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let event_authority = Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0;

    let recent_slot = client.get_slot().expect("get_slot");
    let (create_ix, table_address) = create_lookup_table(payer_address, payer_address, recent_slot);
    let extend_ix = extend_lookup_table(
        table_address,
        payer_address,
        Some(payer_address),
        vec![event_authority],
    );
    let setup = Transaction::new_signed_with_payer(
        &[create_ix, extend_ix],
        Some(&payer_address),
        &[&payer],
        client.get_latest_blockhash().expect("blockhash"),
    );
    client
        .send_and_confirm_transaction(&setup)
        .expect("create and extend the lookup table");

    // A table's addresses can only be used from the slot after they're added.
    let extended_at = client.get_slot().expect("get_slot");
    while client.get_slot().expect("get_slot") <= extended_at {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let table_account = client
        .get_account(&table_address)
        .expect("lookup table account");
    let table = AddressLookupTable::deserialize(&table_account.data).expect("decode lookup table");
    assert_eq!(table.addresses.as_ref(), &[event_authority]);

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ],
        data: EVENT_CPI_EMIT_DISCRIMINATOR.to_vec(),
    };
    let message = v0::Message::try_compile(
        &payer_address,
        &[instruction],
        &[AddressLookupTableAccount {
            key: table_address,
            addresses: vec![event_authority],
        }],
        client.get_latest_blockhash().expect("blockhash"),
    )
    .expect("compile the v0 message");
    assert_eq!(message.address_table_lookups.len(), 1);
    assert!(
        !message.account_keys.contains(&event_authority),
        "event_authority must be reached through the lookup table, not the static keys"
    );
    let transaction = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer])
        .expect("sign the v0 transaction");
    let signature = client
        .send_and_confirm_transaction(&transaction)
        .expect("event_cpi_emit through a lookup table");

    let provider =
        NaclacProvider::new("devnet", payer).expect("Failed to construct NaclacProvider");
    let meta = provider
        .get_transaction(&signature)
        .expect("get_transaction must find the confirmed transaction");

    let inner = meta
        .inner_instructions
        .as_ref()
        .expect("inner instructions must resolve through the lookup table");
    assert_eq!(inner.len(), 2);
    for call in inner {
        assert_eq!(call.program_id, PROGRAM_ID);
        assert_eq!(call.accounts, vec![event_authority]);
    }

    let counters: Vec<CounterIncremented> = meta
        .parse_events_zero_copy()
        .expect("parse_events_zero_copy must not fail");
    assert_eq!(counters.len(), 1);
    assert_eq!(counters[0].new_count, 7);
}
