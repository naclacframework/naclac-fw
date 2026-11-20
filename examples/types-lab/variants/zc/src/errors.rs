use naclac_lang::prelude::*;

#[error_code]
pub enum LabError {
    /// The lab refused this request.
    Rejected,
    /// The counter is already at its maximum.
    Overflowed,
    /// The roster has no room for another entry.
    RosterFull,
}
