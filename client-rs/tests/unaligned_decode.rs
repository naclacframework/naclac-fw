//! `decode_pod_checked` and `decode_pod_unchecked` read account bytes that sit
//! at any address (a `Vec<u8>` only guarantees alignment 1), so they must
//! decode the same value wherever the slice starts, and a packed event must
//! decode through `parse_events_zero_copy`.

use {
    bytemuck::{Pod, Zeroable},
    naclac_client::{
        decode_pod_checked, decode_pod_unchecked, NaclacEvent, NaclacTransactionMetadata,
    },
    solana_address::Address,
    solana_signature::Signature,
};

const DISCRIMINATOR: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Natural {
    big: u64,
    mid: u32,
    small: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Packed {
    flag: u8,
    amount: u64,
    count: u16,
}

impl NaclacEvent for Packed {
    const DISCRIMINATOR: &'static [u8] = &[9, 9];
    const PROGRAM_ID: Address = Address::new_from_array([7; 32]);
    const CPI_TAG: &'static [u8] = &[];
}

fn natural() -> Natural {
    Natural {
        big: 0x0102030405060708,
        mid: 0x090a0b0c,
        small: 0x0d0e0f10,
    }
}

#[test]
fn decode_pod_unchecked_reads_a_naturally_aligned_type_at_every_offset() {
    let bytes = bytemuck::bytes_of(&natural()).to_vec();
    for offset in 0..16 {
        let mut buffer = vec![0u8; offset];
        buffer.extend_from_slice(&bytes);
        let decoded: Natural = decode_pod_unchecked(&buffer[offset..]).unwrap();
        assert_eq!(decoded, natural(), "offset {offset}");
    }
}

#[test]
fn decode_pod_checked_reads_after_the_discriminator_at_every_offset() {
    let mut payload = DISCRIMINATOR.to_vec();
    payload.extend_from_slice(bytemuck::bytes_of(&natural()));
    for offset in 0..16 {
        let mut buffer = vec![0u8; offset];
        buffer.extend_from_slice(&payload);
        let decoded: Natural = decode_pod_checked(&buffer[offset..], &DISCRIMINATOR).unwrap();
        assert_eq!(decoded, natural(), "offset {offset}");
    }
}

#[test]
fn a_wrong_discriminator_or_short_data_is_an_error() {
    let mut payload = DISCRIMINATOR.to_vec();
    payload.extend_from_slice(bytemuck::bytes_of(&natural()));
    assert!(decode_pod_checked::<Natural>(&payload, &[0; 8]).is_err());
    assert!(decode_pod_checked::<Natural>(&payload[..payload.len() - 1], &DISCRIMINATOR).is_err());
    assert!(decode_pod_unchecked::<Natural>(&payload[..8]).is_err());
}

#[test]
fn a_packed_event_decodes_from_a_log_line() {
    use base64::Engine;
    let event = Packed {
        flag: 1,
        amount: 0x1122334455667788,
        count: 0xabcd,
    };
    let mut frame = Packed::DISCRIMINATOR.to_vec();
    frame.extend_from_slice(bytemuck::bytes_of(&event));
    let log = format!(
        "Program data: {}",
        base64::engine::general_purpose::STANDARD.encode(&frame)
    );
    let metadata = NaclacTransactionMetadata {
        signature: Signature::default(),
        logs: vec![log],
        compute_units_consumed: 0,
        return_data: Vec::new(),
        inner_instructions: None,
    };
    let events = metadata.parse_events_zero_copy::<Packed>().unwrap();
    assert_eq!(events.len(), 1);
    let (flag, amount, count) = (events[0].flag, events[0].amount, events[0].count);
    assert_eq!((flag, amount, count), (1, 0x1122334455667788, 0xabcd));
}
