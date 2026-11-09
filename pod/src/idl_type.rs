//! The IDL types of the plain-data types, under `idl-build`. They are the
//! types the IDL has always shown for them: a `Bool` is a `bool`, a
//! `FixedVec<T, N>` the plain `vec` and a `FixedString<N>` the plain `string`
//! (their `N` is the field's `slots`), an `Opt<T>` its own defined struct, and
//! a `ZcEnum<E>` the enum `E` itself.

use {
    crate::{Bool, FixedString, FixedVec, Opt, ZcStorage, ZcString, ZcVec},
    bytemuck::Pod,
    naclac_idl::idl_type::{self as idl, IdlJson, NaclacIdlType, Types},
};

impl NaclacIdlType for Bool {
    fn idl_type() -> IdlJson {
        idl::boolean()
    }
}

impl<T: Pod + NaclacIdlType, const N: usize> NaclacIdlType for FixedVec<T, N> {
    fn idl_type() -> IdlJson {
        idl::vec_of(T::idl_type())
    }

    fn slots() -> Option<u64> {
        Some(N as u64)
    }

    fn chase(types: &mut Types) {
        T::chase(types);
    }
}

impl<const N: usize> NaclacIdlType for FixedString<N> {
    fn idl_type() -> IdlJson {
        idl::string()
    }

    fn slots() -> Option<u64> {
        Some(N as u64)
    }
}

impl<T: Pod + NaclacIdlType> NaclacIdlType for Opt<T> {
    fn idl_type() -> IdlJson {
        idl::opt_of(T::idl_type())
    }

    fn chase(types: &mut Types) {
        T::chase(types);
    }
}

impl<E: NaclacIdlType, const SIZE: usize> NaclacIdlType for ZcStorage<E, SIZE> {
    fn idl_type() -> IdlJson {
        E::idl_type()
    }

    fn chase(types: &mut Types) {
        E::chase(types);
    }
}

impl<T: Pod + NaclacIdlType> NaclacIdlType for ZcVec<'_, T> {
    fn idl_type() -> IdlJson {
        idl::vec_of(T::idl_type())
    }

    fn chase(types: &mut Types) {
        T::chase(types);
    }
}

impl NaclacIdlType for ZcString<'_> {
    fn idl_type() -> IdlJson {
        idl::string()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use {
        super::*,
        crate::{FixedString, FixedVec, Opt},
        naclac_idl::serde_json::json,
        std::{string::String, vec::Vec},
    };

    #[test]
    fn a_fixed_vec_and_string_are_the_plain_vec_and_string() {
        assert!(FixedVec::<u64, 8>::idl_type() == Vec::<u64>::idl_type());
        assert!(FixedVec::<u8, 16>::idl_type() == json!({ "vec": "u8" }));
        assert!(FixedString::<32>::idl_type() == String::idl_type());
        assert!(FixedString::<32>::idl_type() == json!("string"));
    }

    #[test]
    fn the_slot_count_is_the_capacity() {
        assert!(FixedVec::<u64, 8>::slots() == Some(8));
        assert!(FixedString::<32>::slots() == Some(32));
        assert!(<[FixedString<4>; 2]>::slots() == Some(4));
        assert!(<&FixedString<4>>::slots() == Some(4));
    }

    #[test]
    fn a_type_without_a_fixed_part_has_no_slots() {
        assert!(u64::slots().is_none());
        assert!(Vec::<u8>::slots().is_none());
        assert!(String::slots().is_none());
        assert!(<[u8; 32]>::slots().is_none());
    }

    #[test]
    fn an_opt_is_its_own_defined_struct() {
        assert!(Opt::<u64>::idl_type() == idl::opt_of(u64::idl_type()));
        assert!(Opt::<u64>::idl_type().get("defined").is_some());
    }
}
