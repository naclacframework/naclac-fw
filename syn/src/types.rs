//! The parsed shape naclac-syn hands to its callers: a crate constant, as far
//! as resolving array lengths and constant references needs it.

/// A `pub const` or `#[constant]` of the crate: its name and its value, the
/// evaluated number when naclac-syn can evaluate the initializer, otherwise
/// the initializer's source text.
#[derive(Clone, Debug)]
pub struct NaclacConstant {
    pub name: String,
    pub value: String,
}
