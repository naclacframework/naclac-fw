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

type Amount = u64;

#[defined_type]
pub struct Tag {
    pub kind: u8,
    #[max_len(3)]
    pub text: String,
}

#[defined_type]
pub enum Slot {
    Empty,
    Filled(u32),
    Named {
        #[max_len(4)]
        name: String,
        rank: u16,
    },
}

#[component]
pub struct Probe {
    pub amount: Amount,
    #[max_len(6)]
    pub note: Option<String>,
    #[max_len(2, 5)]
    pub names: Vec<String>,
    #[max_len(2)]
    pub tags: Vec<Tag>,
    pub slot: Slot,
    pub grid: [u16; 3],
}

const _: () = assert!(<Tag as SerializedBounds>::MAX_SIZE == 1 + (4 + 3));
const _: () = assert!(<Slot as SerializedBounds>::MAX_SIZE == 1 + (4 + 4) + 2);
const _: () = assert!(
    Probe::SPACE
        == DISCRIMINATOR_LEN
            + 8
            + (1 + (4 + 6))
            + (4 + 2 * (4 + 5))
            + (4 + 2 * (1 + (4 + 3)))
            + (1 + (4 + 4) + 2)
            + 6
);

fn probe_at_max() -> Probe {
    Probe {
        amount: u64::MAX,
        note: Some("n".repeat(6)),
        names: vec!["a".repeat(5); 2],
        tags: vec![
            Tag {
                kind: 1,
                text: "t".repeat(3),
            };
            2
        ],
        slot: Slot::Named {
            name: "s".repeat(4),
            rank: u16::MAX,
        },
        grid: [u16::MAX; 3],
    }
}

#[test]
fn a_probe_at_every_limit_serializes_to_exactly_space() {
    let probe = probe_at_max();
    assert!(probe.within_max_len());
    let mut bytes = Vec::new();
    probe.try_serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), Probe::SPACE);
}

#[test]
fn limits_apply_through_option_vec_and_defined_types() {
    let mut probe = probe_at_max();
    probe.note = Some("n".repeat(7));
    assert!(!probe.within_max_len());

    let mut probe = probe_at_max();
    probe.names[1] = "a".repeat(6);
    assert!(!probe.within_max_len());

    let mut probe = probe_at_max();
    probe.names.push(String::new());
    assert!(!probe.within_max_len());

    let mut probe = probe_at_max();
    probe.tags[0].text = "t".repeat(4);
    assert!(!probe.within_max_len());

    let mut probe = probe_at_max();
    probe.slot = Slot::Named {
        name: "s".repeat(5),
        rank: 0,
    };
    assert!(!probe.within_max_len());

    let mut probe = probe_at_max();
    probe.slot = Slot::Filled(7);
    probe.note = None;
    assert!(probe.within_max_len());
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
