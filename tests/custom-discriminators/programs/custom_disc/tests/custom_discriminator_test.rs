//! End-to-end proof that `#[discriminator = ..]`/`#[component(discriminator = ..)]`/
//! `#[event(discriminator = ..)]` overrides work through the real dispatch,
//! account-loading and event-log paths, not just at the macro/IDL level:
//! - custom-width instruction dispatch actually reaches the right handler
//! - a custom-width account discriminator is written, loaded, and still
//!   rejects a same-owner account of the wrong type in an unconstrained
//!   `mut` slot (the same property `tests/discriminator/` proves for the
//!   default 8-byte width)
//! - a custom-width event discriminator round-trips through the real
//!   `sol_log_data` log output

use naclac_client::*;
use custom_disc_client::{
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
    // Item's discriminator is [9,9,9,9], Marker's is [7,7,7,7] — same width,
    // different value, so the 4-byte custom discriminator is what catches this.
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
