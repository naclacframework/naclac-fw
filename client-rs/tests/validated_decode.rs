//! `decode_pod_checked` rejects stored bytes that fail validation (a length
//! above a list's capacity, a flag byte other than 0 or 1) with an error
//! instead of reading them as another value.

use naclac_client::{decode_pod_checked, Bool, FixedString, FixedVec, NaclacClientError};

const DISCRIMINATOR: [u8; 2] = [7, 7];

fn with_discriminator(body: &[u8]) -> Vec<u8> {
    let mut data = DISCRIMINATOR.to_vec();
    data.extend_from_slice(body);
    data
}

fn assert_rejected<T>(result: Result<T, NaclacClientError>, reason: &str) {
    match result {
        Err(NaclacClientError::DeserializationError(message)) => {
            assert!(message.contains(reason), "{message}");
        }
        Err(_) => panic!("expected a deserialization error"),
        Ok(_) => panic!("expected the stored bytes to be rejected"),
    }
}

#[test]
fn a_list_length_above_its_capacity_is_an_error() {
    let valid = decode_pod_checked::<FixedVec<u8, 3>>(
        &with_discriminator(&[3, 0, 1, 2, 3]),
        &DISCRIMINATOR,
    );
    assert!(valid.is_ok());
    assert_rejected(
        decode_pod_checked::<FixedVec<u8, 3>>(
            &with_discriminator(&[4, 0, 1, 2, 3]),
            &DISCRIMINATOR,
        ),
        "capacity",
    );
}

#[test]
fn a_string_length_above_its_capacity_is_an_error() {
    assert!(decode_pod_checked::<FixedString<2>>(
        &with_discriminator(&[2, 0, b'h', b'i']),
        &DISCRIMINATOR
    )
    .is_ok());
    assert_rejected(
        decode_pod_checked::<FixedString<2>>(
            &with_discriminator(&[3, 0, b'h', b'i']),
            &DISCRIMINATOR,
        ),
        "capacity",
    );
}

#[test]
fn a_flag_byte_other_than_zero_or_one_is_an_error_inside_the_length_only() {
    let unused_slot =
        decode_pod_checked::<FixedVec<Bool, 2>>(&with_discriminator(&[1, 0, 1, 9]), &DISCRIMINATOR);
    assert!(unused_slot.is_ok());
    assert_rejected(
        decode_pod_checked::<FixedVec<Bool, 2>>(&with_discriminator(&[1, 0, 2, 0]), &DISCRIMINATOR),
        "tag",
    );
    assert_rejected(
        decode_pod_checked::<[Bool; 2]>(&with_discriminator(&[1, 2]), &DISCRIMINATOR),
        "tag",
    );
}
