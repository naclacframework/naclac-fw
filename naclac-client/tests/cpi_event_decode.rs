//! Exercises `parse_events_*`'s inner-instruction filter directly on
//! hand-built transaction metadata: only a call to the event's own program,
//! whose first account is that program's `event_authority` PDA and whose data
//! starts with the program's tag, may be decoded as an event.

use bytemuck::{Pod, Zeroable};
use naclac_client::{NaclacEvent, NaclacInnerInstruction, NaclacTransactionMetadata};
use solana_address::Address;
use solana_signature::Signature;

const PROGRAM: Address = Address::new_from_array([7; 32]);
const OTHER_PROGRAM: Address = Address::new_from_array([8; 32]);
const TAG: [u8; 3] = [1, 2, 3];
const DISCRIMINATOR: [u8; 2] = [9, 9];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Counter {
    value: u64,
}

impl NaclacEvent for Counter {
    const DISCRIMINATOR: &'static [u8] = &DISCRIMINATOR;
    const PROGRAM_ID: Address = PROGRAM;
    const CPI_TAG: &'static [u8] = &TAG;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Untagged {
    value: u64,
}

impl NaclacEvent for Untagged {
    const DISCRIMINATOR: &'static [u8] = &DISCRIMINATOR;
    const PROGRAM_ID: Address = PROGRAM;
    const CPI_TAG: &'static [u8] = &[];
}

fn event_authority() -> Address {
    Address::find_program_address(&[b"__event_authority"], &PROGRAM).0
}

fn payload(tag: &[u8], value: u64) -> Vec<u8> {
    let mut data = tag.to_vec();
    data.extend_from_slice(&DISCRIMINATOR);
    data.extend_from_slice(&value.to_le_bytes());
    data
}

fn call(program_id: Address, first_account: Address, data: Vec<u8>) -> NaclacInnerInstruction {
    NaclacInnerInstruction {
        outer_index: 0,
        stack_height: 2,
        program_id,
        accounts: vec![first_account],
        data,
    }
}

fn meta(
    logs: Vec<String>,
    inner_instructions: Option<Vec<NaclacInnerInstruction>>,
) -> NaclacTransactionMetadata {
    NaclacTransactionMetadata {
        signature: Signature::default(),
        logs,
        compute_units_consumed: 0,
        return_data: Vec::new(),
        inner_instructions,
    }
}

fn values(meta: &NaclacTransactionMetadata) -> Vec<u64> {
    meta.parse_events_zero_copy::<Counter>()
        .unwrap()
        .iter()
        .map(|event| event.value)
        .collect()
}

#[test]
fn decodes_a_genuine_self_cpi_event() {
    let meta = meta(
        Vec::new(),
        Some(vec![call(PROGRAM, event_authority(), payload(&TAG, 41))]),
    );
    assert_eq!(values(&meta), vec![41]);
}

#[test]
fn ignores_calls_that_do_not_match_program_authority_and_tag() {
    let forged = vec![
        call(OTHER_PROGRAM, event_authority(), payload(&TAG, 1)),
        call(PROGRAM, Address::new_from_array([5; 32]), payload(&TAG, 2)),
        call(PROGRAM, event_authority(), payload(&[1, 2, 4], 3)),
        call(PROGRAM, event_authority(), payload(&[], 4)),
        NaclacInnerInstruction {
            accounts: Vec::new(),
            ..call(PROGRAM, event_authority(), payload(&TAG, 5))
        },
        call(PROGRAM, event_authority(), TAG.to_vec()),
    ];
    assert_eq!(values(&meta(Vec::new(), Some(forged))), Vec::<u64>::new());
}

#[test]
fn ignores_a_matching_call_whose_discriminator_is_for_another_event() {
    let mut data = TAG.to_vec();
    data.extend_from_slice(&[9, 8]);
    data.extend_from_slice(&6u64.to_le_bytes());
    let meta = meta(
        Vec::new(),
        Some(vec![call(PROGRAM, event_authority(), data)]),
    );
    assert_eq!(values(&meta), Vec::<u64>::new());
}

#[test]
fn unavailable_inner_instructions_yield_no_cpi_events() {
    assert_eq!(values(&meta(Vec::new(), None)), Vec::<u64>::new());
}

#[test]
fn an_event_type_without_a_tag_never_reads_inner_instructions() {
    let meta = meta(
        Vec::new(),
        Some(vec![call(PROGRAM, event_authority(), payload(&TAG, 7))]),
    );
    assert!(meta
        .parse_events_zero_copy::<Untagged>()
        .unwrap()
        .is_empty());
}

#[test]
fn log_events_come_first_then_cpi_events_in_execution_order() {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let log = format!(
        "Program data: {} {}",
        engine.encode(DISCRIMINATOR),
        engine.encode(10u64.to_le_bytes())
    );
    let meta = meta(
        vec![log],
        Some(vec![
            call(PROGRAM, event_authority(), payload(&TAG, 20)),
            call(PROGRAM, event_authority(), payload(&TAG, 30)),
        ]),
    );
    assert_eq!(values(&meta), vec![10, 20, 30]);
}
