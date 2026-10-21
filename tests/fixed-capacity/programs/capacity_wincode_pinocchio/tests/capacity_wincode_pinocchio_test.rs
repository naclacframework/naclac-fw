//! End-to-end LiteSVM tests that the space a `#[max_len]` component is created
//! with holds a roster filled to every `max_len`, and that each `Vec` and
//! `String` is limited to its own `max_len`, not only to the account's total
//! space.

use capacity_wincode_pinocchio_client::{
    fetch_roster, get_roster_pda, instructions::*, types::PROGRAM_ID, Roster,
};
use naclac_client::*;

const DISCRIMINATOR_LEN: usize = 8;
const SPACE: usize = DISCRIMINATOR_LEN + 1 + (4 + 5 * 32) + (4 + 12) + 4;
const CAPACITY_EXCEEDED_AT_ACCOUNT_1: u32 = 10_132;

type Outcome = Result<NaclacTransactionMetadata, NaclacClientError>;

fn assert_custom(result: Outcome, expected: u32) {
    match result {
        Err(NaclacClientError::TransactionFailed {
            instruction_err: InstructionError::Custom(code),
            ..
        }) => assert_eq!(code, expected, "wrong custom error code"),
        other => panic!("expected a Custom({expected}) transaction failure, got {other:?}"),
    }
}

fn setup() -> (NaclacProvider, Address) {
    let payer = load_node_wallet().expect("Failed to load local Solana keypair");
    let provider = NaclacProvider::new("litesvm", payer).expect("Failed to construct provider");
    let (roster, _bump) = get_roster_pda(&PROGRAM_ID);
    build_init_roster(
        &provider,
        PROGRAM_ID,
        InitRosterAccounts {
            payer: provider.payer.address(),
            roster,
            system_program: Address::default(),
        },
    )
    .send_and_confirm()
    .expect("init_roster should succeed");
    (provider, roster)
}

fn fill_roster(provider: &NaclacProvider, roster: Address, count: u8) -> Outcome {
    build_fill_roster(
        provider,
        PROGRAM_ID,
        count,
        FillRosterAccounts {
            signer: provider.payer.address(),
            roster,
        },
    )
    .send_and_confirm()
}

fn set_label(provider: &NaclacProvider, roster: Address, label: &str) -> Outcome {
    build_set_label(
        provider,
        PROGRAM_ID,
        label.to_string(),
        SetLabelAccounts {
            signer: provider.payer.address(),
            roster,
        },
    )
    .send_and_confirm()
}

fn read(provider: &NaclacProvider, roster: &Address) -> Roster {
    fetch_roster(provider, roster).expect("roster should decode")
}

#[test]
fn the_account_is_created_with_exactly_the_computed_space() {
    let (provider, roster) = setup();
    assert_eq!(provider.get_account_data(&roster).unwrap().len(), SPACE);
}

#[test]
fn a_roster_filled_to_every_max_len_fits() {
    let (provider, roster) = setup();
    fill_roster(&provider, roster, 5).expect("five members fit");
    set_label(&provider, roster, "abcdefghijkl").expect("a twelve byte label fits");

    let state = read(&provider, &roster);
    assert_eq!(state.members.len(), 5);
    assert_eq!(state.members[4], Address::new_from_array([5; 32]));
    assert_eq!(state.label, "abcdefghijkl");
    assert_eq!(provider.get_account_data(&roster).unwrap().len(), SPACE);
}

#[test]
fn one_member_more_than_max_len_is_rejected() {
    let (provider, roster) = setup();
    assert_custom(
        fill_roster(&provider, roster, 6),
        CAPACITY_EXCEEDED_AT_ACCOUNT_1,
    );
    assert!(read(&provider, &roster).members.is_empty());
}

#[test]
fn a_label_one_byte_over_max_len_is_rejected_on_a_full_roster() {
    let (provider, roster) = setup();
    fill_roster(&provider, roster, 5).unwrap();
    assert_custom(
        set_label(&provider, roster, "abcdefghijklm"),
        CAPACITY_EXCEEDED_AT_ACCOUNT_1,
    );
    assert!(read(&provider, &roster).label.is_empty());
}

#[test]
fn a_field_cannot_use_room_another_field_left_unused() {
    let (provider, roster) = setup();
    // Only the label is filled, so the account has room for far more than 13
    // bytes, but the label's own max_len is 12.
    set_label(&provider, roster, "abcdefghijkl").expect("a label of exactly max_len fits");
    assert_custom(
        set_label(&provider, roster, "abcdefghijklm"),
        CAPACITY_EXCEEDED_AT_ACCOUNT_1,
    );
    assert_eq!(read(&provider, &roster).label, "abcdefghijkl");
}
