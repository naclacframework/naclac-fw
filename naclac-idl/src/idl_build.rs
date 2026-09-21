//! Runtime-callable trait every `#[component]`/`#[defined_type]`/`#[event]`
//! type implements, only under the `idl-build` feature. Lets a program
//! describe its own real IDL shape via a compiled `#[test]` instead of a
//! second, independent parser guessing it from source text — see
//! docs/plan/idl-build-compilation-migration.md for the full design.

use crate::IdlTypeDef;
use std::collections::BTreeMap;

pub trait NaclacIdlBuild {
    /// Builds this type's IDL shape. `None` means this type is reachable
    /// from the public IDL but doesn't implement this trait — the
    /// `#[program]`-generated print test treats that as a hard failure, not
    /// a silent omission.
    fn create_type() -> Option<IdlTypeDef> {
        None
    }

    /// Inserts every type reachable from this one's own fields/variants
    /// (recursively) into `types`, keyed by [`Self::get_full_path`].
    fn insert_types(_types: &mut BTreeMap<String, IdlTypeDef>) {}

    /// This type's name as it appears in a `{"defined": "..."}` IDL
    /// reference. Always a literal the generating macro already knows at
    /// its own expansion site — never derived from `core::any::type_name`,
    /// whose exact format Rust does not guarantee.
    fn get_full_path() -> String;
}
