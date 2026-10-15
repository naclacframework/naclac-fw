//! A zero-copy `ZcEnum<E>` field has the same IDL type as the plain `E` a
//! Borsh or Wincode program writes, and `Opt<ZcEnum<E>>` names its `Opt` from `E`.

use naclac_syn::parser::{defined_type_leaf, rust_type_to_idl};
use serde_json::json;

fn idl(ty: &str) -> serde_json::Value {
    rust_type_to_idl(&syn::parse_str(ty).unwrap(), &[])
}

#[test]
fn a_zero_copy_enum_field_is_the_plain_enum_in_the_idl() {
    assert_eq!(idl("ZcEnum<Mode>"), idl("Mode"));
    assert_eq!(idl("ZcEnum<Mode>"), json!({ "defined": "Mode" }));
}

#[test]
fn wrappers_around_a_zero_copy_enum_keep_the_enum() {
    assert_eq!(
        idl("[ZcEnum<Mode>; 3]"),
        json!({ "array": [{ "defined": "Mode" }, 3] })
    );
    assert_eq!(idl("Opt<ZcEnum<Mode>>"), json!({ "defined": "OptMode" }));
}

#[test]
fn the_enum_is_still_chased_as_a_defined_type() {
    for ty in ["ZcEnum<Mode>", "[ZcEnum<Mode>; 3]", "Opt<ZcEnum<Mode>>"] {
        let leaf = defined_type_leaf(&syn::parse_str(ty).unwrap()).unwrap();
        assert_eq!(quote::quote!(#leaf).to_string(), "Mode", "{ty}");
    }
}
