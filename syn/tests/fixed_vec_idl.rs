//! `FixedVec<T, N>` and `FixedString<N>` are the plain `vec` and `string` in
//! the IDL, whatever the mode. Their slot count `N` is reported separately, as
//! the field's `slots`, and the element type is still chased as a defined type.

use {
    naclac_syn::parser::{defined_type_leaf, rust_type_slots, rust_type_to_idl},
    serde_json::json,
};

fn idl(ty: &str) -> serde_json::Value {
    rust_type_to_idl(&syn::parse_str(ty).unwrap(), &[])
}

fn slots(ty: &str) -> Option<u64> {
    rust_type_slots(&syn::parse_str(ty).unwrap(), &[])
}

#[test]
fn the_idl_is_the_plain_vec_and_string() {
    assert_eq!(idl("FixedVec<u64, 8>"), idl("Vec<u64>"));
    assert_eq!(idl("FixedVec<Address, 16>"), json!({ "vec": "publicKey" }));
    assert_eq!(idl("FixedVec<u8, 16>"), json!({ "vec": "u8" }));
    assert_eq!(idl("FixedString<32>"), idl("String"));
    assert_eq!(idl("FixedString<32>"), json!("string"));
}

#[test]
fn the_slot_count_is_the_capacity() {
    assert_eq!(slots("FixedVec<u64, 8>"), Some(8));
    assert_eq!(slots("FixedVec<ZcEnum<Mode>, 4>"), Some(4));
    assert_eq!(slots("FixedString<32>"), Some(32));
    assert_eq!(slots("[FixedString<4>; 2]"), Some(4));
}

#[test]
fn a_type_without_a_fixed_part_has_no_slots() {
    assert_eq!(slots("u64"), None);
    assert_eq!(slots("Vec<u8>"), None);
    assert_eq!(slots("String"), None);
    assert_eq!(slots("[u8; 32]"), None);
}

#[test]
fn the_element_type_is_chased_as_a_defined_type() {
    for ty in ["FixedVec<Entry, 4>", "FixedVec<ZcEnum<Entry>, 4>"] {
        let leaf = defined_type_leaf(&syn::parse_str(ty).unwrap()).unwrap();
        assert_eq!(quote::quote!(#leaf).to_string(), "Entry", "{ty}");
    }
    assert!(defined_type_leaf(&syn::parse_str("FixedVec<u8, 4>").unwrap()).is_none());
    assert!(defined_type_leaf(&syn::parse_str("FixedString<4>").unwrap()).is_none());
}

#[test]
#[should_panic(expected = "FixedVec<T, N>")]
fn a_fixed_vec_without_a_capacity_is_rejected() {
    idl("FixedVec<u8>");
}

#[test]
#[should_panic(expected = "FixedVec<T, N>")]
fn a_fixed_vec_without_a_capacity_has_no_slots() {
    slots("FixedVec<u8>");
}
