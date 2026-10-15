//! End-to-end proof that `#[discriminator = ..]`/`#[component(discriminator = ..)]`/
//! `#[event(discriminator = ..)]` overrides work through the real dispatch,
//! account-loading and event-log paths, not just at the macro/IDL level.
//! See `custom_disc`'s (pinocchio) copy of this file for the full rationale
//! — identical here except this exercises the solana-zerocopy backend.

use naclac_client::*;
use custom_disc_zc_client::{
    instructions::{
        build_init_item, build_init_marker, build_touch_item, InitItemAccounts,
        InitMarkerAccounts, TouchItemAccounts,
    },
    get_item_pda, get_marker_pda,
    types::{ItemCreated, PROGRAM_ID},
};

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

fn assert_custom_code(result: Result<NaclacTransactionMetadata, NaclacClientError>, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => {
            assert_eq!(code, expected, "wrong custom error code");
        }
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

#[test]
fn init_and_touch_item_round_trips_through_custom_width_dispatch_and_storage() {
    let provider = setup();
    let (item_pda, _bump) = get_item_pda(&PROGRAM_ID);

    build_init_item(
        &provider,
        PROGRAM_ID,
        InitItemAccounts {
            payer: provider.payer.address(),
            item: item_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_item (1-byte discriminator) should dispatch and succeed");

    build_touch_item(
        &provider,
        PROGRAM_ID,
        TouchItemAccounts {
            caller: provider.payer.address(),
            item: item_pda,
        },
    )
    .send_and_confirm()
    .expect("touch_item on a real Item account should succeed");
}

#[test]
fn rejects_type_confused_account_in_mut_item_slot() {
    let provider = setup();
    let (marker_pda, _bump) = get_marker_pda(&PROGRAM_ID);

    build_init_marker(
        &provider,
        PROGRAM_ID,
        InitMarkerAccounts {
            payer: provider.payer.address(),
            marker: marker_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_marker should succeed");

    let result = build_touch_item(
        &provider,
        PROGRAM_ID,
        TouchItemAccounts {
            caller: provider.payer.address(),
            item: marker_pda,
        },
    )
    .send_and_confirm();

    // `item` is field index 1: 10000 + 1*100 + InvalidAccountDiscriminator (22).
    assert_custom_code(result, 10122);
}

#[test]
fn emitted_event_carries_the_custom_one_byte_discriminator() {
    let provider = setup();
    let (item_pda, _bump) = get_item_pda(&PROGRAM_ID);

    let tx_meta = build_init_item(
        &provider,
        PROGRAM_ID,
        InitItemAccounts {
            payer: provider.payer.address(),
            item: item_pda,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_item should succeed");

    let events: Vec<ItemCreated> = tx_meta
        .parse_events_zero_copy()
        .expect("event log should decode");
    assert_eq!(events.len(), 1, "exactly one ItemCreated event should be emitted");
    assert_eq!(events[0].owner, provider.payer.address());
    assert_eq!(events[0].value, 0);
}
