//! The IDL type of a Rust type, resolved by the compiler. A macro writes
//! `<FieldType as NaclacIdlType>::idl_type()` and never inspects the type's
//! name, so a type alias or an array length given as a constant is the type it
//! names. A type with no implementation is a compile error under `idl-build`.
//!
//! The JSON must equal what the name-based mapper produced: `usize`/`isize`
//! are `u64`/`i64`, `[u8]` is `bytes`, a `FixedVec<T, N>` is the plain `vec`
//! and a `FixedString<N>` the plain `string` (their `N` is the field's
//! `slots`), and a `#[defined_type]` is `{"defined": "<Name>"}`.

use {
    crate::IdlTypeDef,
    serde_json::{json, Value},
    std::collections::BTreeMap,
};

pub use serde_json::Value as IdlJson;

/// The type map `NaclacIdlType::chase` fills in: type name to definition.
pub type Types = BTreeMap<String, IdlTypeDef>;

/// The `bool` type, for the one-byte `Bool` of `naclac-pod`.
pub fn boolean() -> Value {
    json!("bool")
}

/// The `publicKey` type, for an address type defined outside this crate.
pub fn public_key() -> Value {
    json!("publicKey")
}

/// The `string` type, for `FixedString`, `ZcString` and the like.
pub fn string() -> Value {
    json!("string")
}

/// A `vec` of `inner`, for `FixedVec`, `ZcVec` and the like.
pub fn vec_of(inner: Value) -> Value {
    json!({ "vec": inner })
}

/// The defined struct that describes `Opt<T>` for the IDL type `inner` of `T`.
/// Panics for an `inner` that `Opt<T>` cannot hold, with a message naming the
/// requirement.
pub fn opt_of(inner: Value) -> Value {
    match naclac_syn::opt_type::opt_type_name(&inner) {
        Some(name) => json!({ "defined": name }),
        None => panic!(
            "Naclac Error: `Opt<T>` has no IDL representation for this `T`: it needs a fixed-size \
             `T` (a primitive, `Address`, an array or a defined type)."
        ),
    }
}

/// The reference to the defined type `name`.
pub fn defined(name: &str) -> Value {
    json!({ "defined": name })
}

/// The IDL entry of a constant `value` named `name`: its type from
/// `NaclacIdlType` and its text from `NaclacIdlType::idl_value`.
pub fn constant_of<T: NaclacIdlType + ?Sized>(
    name: &str,
    docs: Vec<String>,
    value: &T,
) -> crate::IdlConstant {
    crate::IdlConstant {
        name: name.into(),
        docs,
        ty: T::idl_type(),
        value: value.idl_value(),
    }
}

#[diagnostic::on_unimplemented(
    message = "Naclac Error: `{Self}` has no IDL type, so it cannot appear in the IDL.",
    label = "no IDL type",
    note = "use a primitive, Address, String, array, Vec, Option, FixedVec, or a #[defined_type]/#[account] type"
)]
pub trait NaclacIdlType {
    /// The type's JSON in the IDL.
    fn idl_type() -> Value;

    /// The value's text in an IDL constant: the `Debug` text of a primitive,
    /// string or collection, and base58 for an address. Panics for a type that
    /// cannot be an IDL constant.
    fn idl_value(&self) -> String {
        panic!(
            "Naclac Error: `{}` cannot be the type of an IDL constant. Use a primitive, string, \
             array, Vec, Option or Address.",
            std::any::type_name::<Self>()
        )
    }

    /// The `N` of the `FixedVec<T, N>` or `FixedString<N>` the type is, or is
    /// an array or reference of; `None` when it holds neither.
    fn slots() -> Option<u64> {
        None
    }

    /// The most elements or bytes an instruction argument of this type may
    /// carry (`ZcVec<T, N>`, `ZcString<N>`); `None` for no limit.
    fn max_len() -> Option<u64> {
        None
    }

    /// Inserts into `types` the definition of every defined type the type
    /// reaches through `Option`, `Vec`, arrays, references and the like.
    fn chase(_types: &mut BTreeMap<String, IdlTypeDef>) {}
}

macro_rules! named {
    ($($ty:ty => $name:literal),* $(,)?) => {
        $(
            impl NaclacIdlType for $ty {
                fn idl_type() -> Value {
                    json!($name)
                }

                fn idl_value(&self) -> String {
                    format!("{self:?}")
                }
            }
        )*
    };
}

named!(
    u8 => "u8", u16 => "u16", u32 => "u32", u64 => "u64", u128 => "u128",
    i8 => "i8", i16 => "i16", i32 => "i32", i64 => "i64", i128 => "i128",
    f32 => "f32", f64 => "f64",
    // Solana programs are 64-bit, so these are 8 bytes on the wire.
    usize => "u64", isize => "i64",
    bool => "bool",
    String => "string", str => "string",
);

impl NaclacIdlType for solana_address::Address {
    fn idl_type() -> Value {
        json!("publicKey")
    }

    fn idl_value(&self) -> String {
        self.to_string()
    }
}

fn list_text<'a, T: NaclacIdlType + 'a>(items: impl Iterator<Item = &'a T>) -> String {
    let parts: Vec<String> = items.map(NaclacIdlType::idl_value).collect();
    format!("[{}]", parts.join(", "))
}

impl<T: NaclacIdlType + ?Sized> NaclacIdlType for &T {
    fn idl_type() -> Value {
        T::idl_type()
    }

    fn idl_value(&self) -> String {
        (**self).idl_value()
    }

    fn slots() -> Option<u64> {
        T::slots()
    }

    fn max_len() -> Option<u64> {
        T::max_len()
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

impl<T: NaclacIdlType + ?Sized> NaclacIdlType for Box<T> {
    fn idl_type() -> Value {
        T::idl_type()
    }

    fn idl_value(&self) -> String {
        (**self).idl_value()
    }

    fn slots() -> Option<u64> {
        T::slots()
    }

    fn max_len() -> Option<u64> {
        T::max_len()
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

impl<T: NaclacIdlType> NaclacIdlType for Option<T> {
    fn idl_type() -> Value {
        json!({ "option": T::idl_type() })
    }

    fn idl_value(&self) -> String {
        match self {
            Some(inner) => format!("Some({})", inner.idl_value()),
            None => "None".into(),
        }
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

impl<T: NaclacIdlType> NaclacIdlType for Vec<T> {
    fn idl_type() -> Value {
        json!({ "vec": T::idl_type() })
    }

    fn idl_value(&self) -> String {
        list_text(self.iter())
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

impl<T: NaclacIdlType> NaclacIdlType for [T] {
    fn idl_type() -> Value {
        let inner = T::idl_type();
        if inner == json!("u8") {
            return json!("bytes");
        }
        json!({ "vec": inner })
    }

    fn idl_value(&self) -> String {
        list_text(self.iter())
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

impl<T: NaclacIdlType, const N: usize> NaclacIdlType for [T; N] {
    fn idl_type() -> Value {
        json!({ "array": [T::idl_type(), N] })
    }

    fn idl_value(&self) -> String {
        list_text(self.iter())
    }

    fn slots() -> Option<u64> {
        T::slots()
    }

    fn chase(types: &mut BTreeMap<String, IdlTypeDef>) {
        T::chase(types);
    }
}

/// Inserts the definition of the defined type `T` and everything it reaches
/// into `types`, keyed by its name. Used by the `NaclacIdlType::chase` impl of
/// every `#[account]`, `#[defined_type]` and `#[instruction_args]` type.
pub fn chase_defined<T: crate::idl_build::NaclacIdlBuild>(
    types: &mut BTreeMap<String, IdlTypeDef>,
) {
    if let Some(definition) = T::create_type() {
        types.insert(T::get_full_path(), definition);
        T::insert_types(types);
    }
}

#[cfg(test)]
mod tests {
    use super::NaclacIdlType;

    type Amount = u64;
    const LEN: usize = 3;

    #[test]
    fn primitives_and_wrappers_match_the_old_mapper() {
        assert!(<usize as NaclacIdlType>::idl_type() == serde_json::json!("u64"));
        assert!(<Amount as NaclacIdlType>::idl_type() == serde_json::json!("u64"));
        assert!(<&str as NaclacIdlType>::idl_type() == serde_json::json!("string"));
        assert!(<&[u8] as NaclacIdlType>::idl_type() == serde_json::json!("bytes"));
        assert!(<&[u16] as NaclacIdlType>::idl_type() == serde_json::json!({ "vec": "u16" }));
        assert!(<Vec<u8> as NaclacIdlType>::idl_type() == serde_json::json!({ "vec": "u8" }));
        assert!(
            <Option<Vec<String>> as NaclacIdlType>::idl_type()
                == serde_json::json!({ "option": { "vec": "string" } })
        );
        assert!(
            <solana_address::Address as NaclacIdlType>::idl_type()
                == serde_json::json!("publicKey")
        );
    }

    #[test]
    fn a_constant_takes_its_type_and_text_from_the_value() {
        let text = super::constant_of("NAME", vec![], &"abc");
        assert!(text.ty == serde_json::json!("string") && text.value == "\"abc\"");
        let bytes = super::constant_of("SEED", vec![], &[1u8, 2]);
        assert!(bytes.ty == serde_json::json!({ "array": ["u8", 2] }) && bytes.value == "[1, 2]");
        let address = super::constant_of("ID", vec![], &solana_address::Address::default());
        assert!(address.ty == serde_json::json!("publicKey"));
        assert!(address.value == "11111111111111111111111111111111");
    }

    #[test]
    fn an_array_length_given_as_a_constant_is_its_value() {
        assert!(
            <[u8; LEN] as NaclacIdlType>::idl_type() == serde_json::json!({ "array": ["u8", 3] })
        );
        assert!(
            <[[u8; 2]; LEN] as NaclacIdlType>::idl_type()
                == serde_json::json!({ "array": [{ "array": ["u8", 2] }, 3] })
        );
    }
}
