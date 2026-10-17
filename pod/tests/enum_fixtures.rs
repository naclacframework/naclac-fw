//! The byte strings of `client/tests/enum-differential.test.ts`, asserted from
//! the Rust side: a data-carrying enum is stored as its tag (the `repr`,
//! little-endian), the active variant's fields with no padding, then zeros up
//! to the size of the largest variant. The TypeScript test asserts the same
//! strings, so both codecs are tied to one set of bytes.

use naclac_pod::ZcCodec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InnerMode {
    A,
    B,
}

impl ZcCodec for InnerMode {
    const SIZE: usize = 1;

    fn encode(&self, out: &mut [u8]) {
        out[0] = match self {
            InnerMode::A => 0,
            InnerMode::B => 1,
        };
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes.first()? {
            0 => Some(InnerMode::A),
            1 => Some(InnerMode::B),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StepMode {
    Simple,
    Stepped(u64),
    Named { rate: u32, mode: InnerMode },
}

impl ZcCodec for StepMode {
    const SIZE: usize = 1 + 8;

    fn encode(&self, out: &mut [u8]) {
        match self {
            StepMode::Simple => out[0] = 0,
            StepMode::Stepped(value) => {
                out[0] = 1;
                out[1..9].copy_from_slice(&value.to_le_bytes());
            }
            StepMode::Named { rate, mode } => {
                out[0] = 10;
                out[1..5].copy_from_slice(&rate.to_le_bytes());
                mode.encode(&mut out[5..6]);
            }
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..Self::SIZE)?;
        match bytes[0] {
            0 => Some(StepMode::Simple),
            1 => Some(StepMode::Stepped(u64::from_le_bytes(
                bytes[1..9].try_into().ok()?,
            ))),
            10 => Some(StepMode::Named {
                rate: u32::from_le_bytes(bytes[1..5].try_into().ok()?),
                mode: InnerMode::decode(&bytes[5..6])?,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WideMode {
    Alpha,
    Beta(u32),
}

impl ZcCodec for WideMode {
    const SIZE: usize = 2 + 4;

    fn encode(&self, out: &mut [u8]) {
        match self {
            WideMode::Alpha => out[..2].copy_from_slice(&0u16.to_le_bytes()),
            WideMode::Beta(value) => {
                out[..2].copy_from_slice(&1u16.to_le_bytes());
                out[2..6].copy_from_slice(&value.to_le_bytes());
            }
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..Self::SIZE)?;
        match u16::from_le_bytes([bytes[0], bytes[1]]) {
            0 => Some(WideMode::Alpha),
            1 => Some(WideMode::Beta(u32::from_le_bytes(
                bytes[2..6].try_into().ok()?,
            ))),
            _ => None,
        }
    }
}

fn stored<T: ZcCodec>(value: &T) -> String {
    let mut bytes = vec![0u8; T::SIZE];
    value.encode(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
        .collect()
}

#[test]
fn step_mode_matches_the_shared_byte_strings() {
    let cases = [
        (StepMode::Simple, "0".repeat(18)),
        (StepMode::Stepped(42), "012a00000000000000".to_string()),
        (
            StepMode::Named {
                rate: 7,
                mode: InnerMode::B,
            },
            "0a0700000001000000".to_string(),
        ),
        (
            StepMode::Named {
                rate: u32::MAX,
                mode: InnerMode::A,
            },
            "0affffffff00000000".to_string(),
        ),
    ];
    for (value, hex) in cases {
        assert_eq!(stored(&value), hex);
        assert_eq!(StepMode::decode(&from_hex(&hex)), Some(value));
    }
}

#[test]
fn wide_mode_matches_the_shared_byte_strings() {
    let cases = [
        (WideMode::Alpha, "000000000000"),
        (WideMode::Beta(99), "010063000000"),
        (WideMode::Beta(u32::MAX), "0100ffffffff"),
    ];
    for (value, hex) in cases {
        assert_eq!(stored(&value), hex);
        assert_eq!(WideMode::decode(&from_hex(hex)), Some(value));
    }
}

#[test]
fn a_corrupted_nested_tag_is_rejected() {
    let mut bytes = from_hex("0a0700000001000000");
    bytes[5] = 5;
    assert_eq!(StepMode::decode(&bytes), None);
}

#[test]
fn every_step_mode_tag_is_accepted_exactly_when_it_names_a_variant() {
    for tag in 0..=255u8 {
        let mut bytes = [0u8; 9];
        bytes[0] = tag;
        assert_eq!(
            StepMode::decode(&bytes).is_some(),
            matches!(tag, 0 | 1 | 10),
            "tag {tag}"
        );
    }
}

#[test]
fn every_wide_mode_tag_is_accepted_exactly_when_it_names_a_variant() {
    for tag in 0..=u16::MAX {
        let mut bytes = [0u8; 6];
        bytes[..2].copy_from_slice(&tag.to_le_bytes());
        assert_eq!(
            WideMode::decode(&bytes).is_some(),
            matches!(tag, 0 | 1),
            "tag {tag}"
        );
    }
}
