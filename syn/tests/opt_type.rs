//! Pins the IDL names of naclac's `Opt<T>` and that naming and parsing them
//! are exact inverses.

use {
    naclac_syn::opt_type::{opt_fields, opt_inner_type, opt_type_name},
    serde_json::{json, Value},
};

#[test]
fn the_idl_struct_is_a_tag_byte_then_the_value() {
    assert_eq!(
        opt_fields(json!("u64")).to_vec(),
        vec![("tag", json!("u8")), ("value", json!("u64"))]
    );
}

fn defined(name: &str) -> bool {
    matches!(name, "Pair" | "Box2x3")
}

#[test]
fn names_are_opt_plus_the_inner_type() {
    let cases: [(Value, &str); 7] = [
        (json!("u64"), "OptU64"),
        (json!("u32"), "OptU32"),
        (json!("i128"), "OptI128"),
        (json!("bool"), "OptBool"),
        (json!("publicKey"), "OptAddress"),
        (json!({ "defined": "Pair" }), "OptPair"),
        (json!({ "array": ["u8", 32] }), "OptU8x32"),
    ];
    for (inner, name) in cases {
        assert_eq!(opt_type_name(&inner).as_deref(), Some(name));
        assert_eq!(opt_inner_type(name, defined), Some(inner));
    }
}

#[test]
fn nested_arrays_and_defined_names_with_x_round_trip() {
    for inner in [
        json!({ "array": [{ "array": ["u16", 2] }, 3] }),
        json!({ "defined": "Box2x3" }),
        json!({ "array": [{ "defined": "Pair" }, 4] }),
    ] {
        let name = opt_type_name(&inner).unwrap();
        assert_eq!(opt_inner_type(&name, defined), Some(inner), "{name}");
    }
}

#[test]
fn types_opt_cannot_hold_have_no_name_and_unrelated_names_do_not_parse() {
    assert_eq!(opt_type_name(&json!("string")), None);
    assert_eq!(opt_type_name(&json!({ "vec": "u8" })), None);
    assert_eq!(opt_type_name(&json!({ "option": "u8" })), None);
    assert_eq!(opt_inner_type("Optional", defined), None);
    assert_eq!(opt_inner_type("Pair", defined), None);
    assert_eq!(opt_inner_type("OptUnknown", defined), None);
}
