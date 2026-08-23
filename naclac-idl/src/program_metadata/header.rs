//! Parses a metadata account's raw on-chain bytes — the read-side
//! counterpart to `instructions.rs`. Same rigor as the rest of this module:
//! byte offsets verified directly against `program-metadata`'s own
//! `state::header::Header` struct layout, not the generated client's Borsh
//! derive (which was never independently verified and is exactly the kind
//! of custom-wrapper code this module already found one real bug in).

use solana_address::Address;

use super::Seed;

/// Real on-chain size of a `Header`/`Buffer` account — see [`super::HEADER_LEN`].
const HEADER_LEN: usize = super::HEADER_LEN;

/// A metadata account's discriminator — distinguishes an empty account, an
/// in-progress `allocate`/`write` buffer not yet finalized by [`super::initialize`],
/// and a real, finalized metadata account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountDiscriminator {
    Empty,
    Buffer,
    Metadata,
}

impl AccountDiscriminator {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Empty),
            1 => Some(Self::Buffer),
            2 => Some(Self::Metadata),
            _ => None,
        }
    }
}

/// A parsed metadata account header — the fixed-size prefix every metadata
/// account starts with, immediately followed by its payload bytes.
#[derive(Clone, Debug)]
pub struct MetadataHeader {
    pub discriminator: AccountDiscriminator,
    pub program: Address,
    /// `None` for a canonical metadata account with no override — the
    /// program's own upgrade authority still manages it either way.
    pub authority: Option<Address>,
    pub mutable: bool,
    pub canonical: bool,
    pub seed: Seed,
    pub encoding: u8,
    pub compression: u8,
    pub format: u8,
    pub data_source: u8,
    pub data_length: u32,
}

/// Parses a metadata account's raw bytes into its header. Returns `None` if
/// `data` is too short or its discriminator byte isn't one of the three
/// real values — never guesses at a partial/corrupt account.
pub fn parse_header(data: &[u8]) -> Option<MetadataHeader> {
    if data.len() < HEADER_LEN {
        return None;
    }

    let discriminator = AccountDiscriminator::from_byte(data[0])?;
    let program = Address::new_from_array(data[1..33].try_into().ok()?);

    let authority_bytes: [u8; 32] = data[33..65].try_into().ok()?;
    let authority = if authority_bytes == [0u8; 32] {
        None
    } else {
        Some(Address::new_from_array(authority_bytes))
    };

    let mutable = data[65] != 0;
    let canonical = data[66] != 0;
    let seed: Seed = data[67..83].try_into().ok()?;
    let encoding = data[83];
    let compression = data[84];
    let format = data[85];
    let data_source = data[86];
    let data_length = u32::from_le_bytes(data[87..91].try_into().ok()?);

    Some(MetadataHeader {
        discriminator,
        program,
        authority,
        mutable,
        canonical,
        seed,
        encoding,
        compression,
        format,
        data_source,
        data_length,
    })
}

/// Returns the payload bytes following `header` in `data` — the raw
/// (still-compressed, if `header.compression` says so) content. `None` if
/// `data` doesn't actually contain `header.data_length` bytes past the
/// header (a truncated/corrupt account).
pub fn payload<'a>(data: &'a [u8], header: &MetadataHeader) -> Option<&'a [u8]> {
    data.get(HEADER_LEN..HEADER_LEN + header.data_length as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bytes(discriminator: u8, data_length: u32, trailing: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0] = discriminator;
        bytes[1..33].copy_from_slice(&[7u8; 32]); // program
                                                  // authority left zero (None)
        bytes[65] = 1; // mutable
        bytes[66] = 1; // canonical
        bytes[67..83].copy_from_slice(b"idl\0\0\0\0\0\0\0\0\0\0\0\0\0");
        bytes[83] = 1; // encoding: Utf8
        bytes[84] = 2; // compression: Zlib
        bytes[85] = 1; // format: Json
        bytes[86] = 0; // data_source: Direct
        bytes[87..91].copy_from_slice(&data_length.to_le_bytes());
        bytes.extend_from_slice(trailing);
        bytes
    }

    #[test]
    fn parses_a_well_formed_metadata_header() {
        let payload_bytes = b"compressed-idl-bytes";
        let bytes = sample_bytes(2, payload_bytes.len() as u32, payload_bytes);

        let header = parse_header(&bytes).expect("should parse");
        assert_eq!(header.discriminator, AccountDiscriminator::Metadata);
        assert_eq!(header.program, Address::new_from_array([7u8; 32]));
        assert_eq!(header.authority, None);
        assert!(header.mutable);
        assert!(header.canonical);
        assert_eq!(header.encoding, 1);
        assert_eq!(header.compression, 2);
        assert_eq!(header.format, 1);
        assert_eq!(header.data_source, 0);
        assert_eq!(header.data_length, payload_bytes.len() as u32);

        assert_eq!(payload(&bytes, &header), Some(&payload_bytes[..]));
    }

    #[test]
    fn rejects_too_short_data() {
        assert!(parse_header(&[0u8; 10]).is_none());
    }

    #[test]
    fn rejects_invalid_discriminator() {
        let bytes = sample_bytes(9, 0, &[]);
        assert!(parse_header(&bytes).is_none());
    }

    #[test]
    fn payload_returns_none_when_truncated() {
        let bytes = sample_bytes(2, 100, b"too-short");
        let header = parse_header(&bytes).unwrap();
        assert_eq!(payload(&bytes, &header), None);
    }
}
