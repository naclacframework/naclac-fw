//! Host-side checks of the `SPACE` the `#[component]` macro computes from `#[max_len]`.

use crate::components::Roster;
use naclac_lang::prelude::*;

const DISCRIMINATOR_LEN: usize = 8;
const MEMBERS_MAX: usize = 5;
const LABEL_MAX: usize = 12;

const _: () = assert!(Roster::SPACE >= DISCRIMINATOR_LEN + 4 + MEMBERS_MAX * 32);

const _: () = assert!(
    Roster::SPACE == DISCRIMINATOR_LEN + 1 + (4 + MEMBERS_MAX * 32) + (4 + LABEL_MAX) + 4
);

fn roster(members: usize, label: usize) -> Roster {
    Roster {
        bump: 0,
        members: core::iter::repeat_n(Address::new_from_array([1; 32]), members).collect(),
        label: "x".repeat(label),
        counter: 0,
    }
}

#[test]
fn each_field_is_checked_against_its_own_max_len() {
    assert!(roster(0, 0).within_max_len());
    assert!(roster(MEMBERS_MAX, LABEL_MAX).within_max_len());
    assert!(!roster(MEMBERS_MAX + 1, 0).within_max_len());
    assert!(!roster(0, LABEL_MAX + 1).within_max_len());
}

#[test]
fn a_roster_at_max_len_serializes_to_exactly_space() {
    let roster = Roster {
        bump: u8::MAX,
        members: core::iter::repeat_n(Address::new_from_array([1; 32]), MEMBERS_MAX).collect(),
        label: "x".repeat(LABEL_MAX),
        counter: u32::MAX,
    };
    let mut bytes = Vec::new();
    roster.try_serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), Roster::SPACE);
}
