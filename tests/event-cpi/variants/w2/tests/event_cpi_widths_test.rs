//! `#[event_cpi]` with non-default discriminator widths. The reserved tag takes
//! the program's *instruction* width (2 here) and the event keeps its own
//! (7 here); the two are separate fields of the self-CPI payload, so they
//! must never be confused. Also proves a handler without `#[event_cpi]` still
//! works beside the ones that have it, and that two handlers sharing one
//! `#[event_cpi]` accounts struct both emit.

use event_cpi_widths_zc_client::{
    instructions::{
        build_emit_a, build_emit_b, build_quiet, EmitAAccounts, EmitBAccounts, QuietAccounts,
    },
    types::{Ticked, PROGRAM_ID},
};
use naclac_client::*;

/// `naclac_syn::discriminator::event_dispatch_tag(2)`.
const TAG: [u8; 2] = [34, 131];
const EVENT_DISCRIMINATOR: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];

fn setup() -> NaclacProvider {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    NaclacProvider::new("litesvm", payer).expect("Failed to construct NaclacProvider")
}

fn events_of(meta: &NaclacTransactionMetadata) -> Vec<Ticked> {
    #[cfg(feature = "bytemuck")]
    let events = meta.parse_events_zero_copy();
    #[cfg(feature = "borsh")]
    let events = meta.parse_events_borsh();
    #[cfg(feature = "wincode")]
    let events = meta.parse_events_wincode();
    events.expect("the transaction's events must parse")
}

fn event_authority() -> Address {
    Address::find_program_address(&[b"__event_authority"], &PROGRAM_ID).0
}

fn error_text<T: std::fmt::Debug, E: std::fmt::Debug>(result: Result<T, E>) -> String {
    format!("{:?}", result.expect_err("expected the transaction to fail"))
}

#[test]
fn the_tag_and_the_event_discriminator_each_use_their_own_width() {
    let provider = setup();
    let meta = build_emit_a(
        &provider,
        PROGRAM_ID,
        EmitAAccounts {
            event_authority: event_authority(),
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emit_a should succeed");

    let inner = meta
        .inner_instructions
        .as_ref()
        .expect("litesvm must supply inner instructions");
    assert_eq!(inner.len(), 1);
    let data = &inner[0].data;
    assert_eq!(data.len(), TAG.len() + EVENT_DISCRIMINATOR.len() + 8);
    assert_eq!(&data[..2], &TAG[..]);
    assert_eq!(&data[2..9], &EVENT_DISCRIMINATOR[..]);
    assert_eq!(&data[9..], &11u64.to_le_bytes()[..]);

    let events = events_of(&meta);
    assert_eq!(events.len(), 1);
    let ticked = events[0].n;
    assert_eq!(ticked, 11);
}

#[test]
fn two_handlers_sharing_one_accounts_struct_both_emit() {
    let provider = setup();
    let first = build_emit_a(
        &provider,
        PROGRAM_ID,
        EmitAAccounts {
            event_authority: event_authority(),
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emit_a should succeed");
    let second = build_emit_b(
        &provider,
        PROGRAM_ID,
        EmitBAccounts {
            event_authority: event_authority(),
            program: PROGRAM_ID,
        },
    )
    .send_and_confirm()
    .expect("emit_b should succeed");

    let first_events = events_of(&first);
    let second_events = events_of(&second);
    assert_eq!(first_events.iter().map(|e| e.n).collect::<Vec<_>>(), vec![11]);
    assert_eq!(second_events.iter().map(|e| e.n).collect::<Vec<_>>(), vec![22]);
}

#[test]
fn a_handler_without_event_cpi_works_beside_the_ones_that_have_it() {
    let provider = setup();
    let meta = build_quiet(
        &provider,
        PROGRAM_ID,
        QuietAccounts {
            caller: provider.payer.address(),
        },
    )
    .send_and_confirm()
    .expect("quiet should succeed");
    assert_eq!(meta.inner_instructions.as_ref().map(Vec::len), Some(0));
    let events = events_of(&meta);
    assert!(events.is_empty());
}

#[test]
fn the_reserved_tag_is_matched_at_the_instruction_width() {
    let provider = setup();

    // The tag alone reaches the receive arm, which then finds no authority.
    let recognized =
        error_text(InstructionBuilder::new(&provider, PROGRAM_ID, TAG.to_vec()).send_and_confirm());
    assert!(recognized.contains("Custom(10016)"), "{recognized}");

    // One byte off is not the tag, and is not a real instruction either.
    let mut off_by_one = TAG.to_vec();
    off_by_one[1] ^= 0xFF;
    let unknown =
        error_text(InstructionBuilder::new(&provider, PROGRAM_ID, off_by_one).send_and_confirm());
    assert!(unknown.contains("Custom(10012)"), "{unknown}");
}
