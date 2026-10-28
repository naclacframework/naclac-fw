//! Compile-time validation of a program address written as a base58 string,
//! used by `declare_id!`. The decoder behind `address!` accepts strings that
//! are not the canonical encoding of 32 bytes (a string of 41 `1`s decodes to
//! the all-zero address), so the address a program declares is checked here
//! as well. Evaluated only in a `const` item: it costs nothing at runtime.

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

const fn digit_of(character: u8) -> u8 {
    let mut index = 0;
    while index < ALPHABET.len() {
        if ALPHABET[index] == character {
            return index as u8;
        }
        index += 1;
    }
    panic!("`declare_id!`: the address contains a character that is not base58");
}

/// Panics unless `address` is the canonical base58 encoding of exactly 32
/// bytes: every character is base58, the value fits in 32 bytes, and the
/// number of leading `1` characters equals the number of leading zero bytes
/// (which also fixes the byte count at 32).
pub const fn assert_program_address(address: &str) {
    let characters = address.as_bytes();
    // `value[0]` stays zero unless the number needs more than 32 bytes.
    let mut value = [0u8; 33];

    let mut position = 0;
    while position < characters.len() {
        let mut carry = digit_of(characters[position]) as u32;
        let mut index = 32;
        loop {
            let product = value[index] as u32 * 58 + carry;
            value[index] = (product & 0xff) as u8;
            carry = product >> 8;
            if index == 0 {
                break;
            }
            index -= 1;
        }
        assert!(
            carry == 0 && value[0] == 0,
            "`declare_id!`: the address is longer than 32 bytes"
        );
        position += 1;
    }

    let mut leading_ones = 0;
    while leading_ones < characters.len() && characters[leading_ones] == b'1' {
        leading_ones += 1;
    }
    let mut leading_zeros = 0;
    while leading_zeros < 32 && value[1 + leading_zeros] == 0 {
        leading_zeros += 1;
    }
    assert!(
        leading_ones == leading_zeros,
        "`declare_id!`: the address is not the canonical base58 encoding of 32 bytes (its \
         leading `1` characters must match its leading zero bytes)"
    );
}

#[cfg(test)]
mod tests {
    use super::assert_program_address;

    #[test]
    fn real_addresses_are_accepted() {
        for address in [
            "11111111111111111111111111111111",
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        ] {
            assert_program_address(address);
        }
    }

    #[test]
    #[should_panic(expected = "not the canonical base58 encoding")]
    fn too_many_leading_ones_are_rejected() {
        assert_program_address("11111111111111111111111111111111111111111");
    }

    #[test]
    #[should_panic(expected = "not the canonical base58 encoding")]
    fn a_value_shorter_than_32_bytes_is_rejected() {
        assert_program_address("2");
    }

    #[test]
    #[should_panic(expected = "not the canonical base58 encoding")]
    fn an_empty_address_is_rejected() {
        assert_program_address("");
    }

    #[test]
    #[should_panic(expected = "longer than 32 bytes")]
    fn a_value_over_32_bytes_is_rejected() {
        assert_program_address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DAzzz");
    }

    #[test]
    #[should_panic(expected = "not base58")]
    fn a_character_outside_base58_is_rejected() {
        assert_program_address("0okenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    }
}
