//! naclac's zero-copy `Opt<T>` in the IDL. `Opt<T>` is generic, so each
//! instantiation is described as its own defined struct, named `Opt` plus the
//! inner type's name: `OptU64`, `OptBool`, `OptAddress`, `OptPair`, and
//! `OptU8x32` for `Opt<[u8; 32]>`. [`opt_type_name`] and [`opt_inner_type`]
//! must stay exact inverses: the IDL generators name a reference with the
//! first and define it, in `naclac_idl::Idl::canonicalize`, with the second.

use serde_json::{json, Value};

const PRIMITIVES: [(&str, &str); 14] = [
    ("u8", "U8"),
    ("u16", "U16"),
    ("u32", "U32"),
    ("u64", "U64"),
    ("u128", "U128"),
    ("i8", "I8"),
    ("i16", "I16"),
    ("i32", "I32"),
    ("i64", "I64"),
    ("i128", "I128"),
    ("f32", "F32"),
    ("f64", "F64"),
    ("bool", "Bool"),
    ("publicKey", "Address"),
];

/// The IDL name of `Opt<T>` for the IDL type `inner` of `T`. `None` for an
/// inner type `Opt<T>` cannot hold (`T` must be `Pod`: no string, vec or
/// option).
pub fn opt_type_name(inner: &Value) -> Option<String> {
    inner_name(inner).map(|name| format!("Opt{name}"))
}

fn inner_name(ty: &Value) -> Option<String> {
    match ty {
        Value::String(s) => PRIMITIVES
            .iter()
            .find(|(idl, _)| idl == s)
            .map(|(_, name)| (*name).to_string()),
        Value::Object(o) => {
            if let Some(Value::String(defined)) = o.get("defined") {
                return Some(defined.clone());
            }
            let array = o.get("array")?.as_array()?;
            let len = array.get(1)?.as_u64()?;
            Some(format!("{}x{len}", inner_name(array.first()?)?))
        }
        _ => None,
    }
}

/// The IDL type `T` of the `Opt<T>` named `name`, when `name` is such a
/// name. `is_defined` says whether a name is one of the IDL's defined types.
pub fn opt_inner_type(name: &str, is_defined: impl Fn(&str) -> bool) -> Option<Value> {
    parse_inner(name.strip_prefix("Opt")?, &is_defined)
}

fn parse_inner(name: &str, is_defined: &dyn Fn(&str) -> bool) -> Option<Value> {
    if let Some((idl, _)) = PRIMITIVES.iter().find(|(_, n)| *n == name) {
        return Some(json!(idl));
    }
    if let Some((elem, len)) = name.rsplit_once('x') {
        if !len.is_empty() && len.bytes().all(|b| b.is_ascii_digit()) {
            if let (Some(elem), Ok(len)) = (parse_inner(elem, is_defined), len.parse::<u64>()) {
                return Some(json!({ "array": [elem, len] }));
            }
        }
    }
    is_defined(name).then(|| json!({ "defined": name }))
}

/// The fields of `Opt<T>`'s `#[repr(C, packed)]` layout in naclac-pod:
/// `tag: u8` (1 when set), then `value: T`.
pub fn opt_fields(inner: Value) -> [(&'static str, Value); 2] {
    [("tag", json!("u8")), ("value", inner)]
}
