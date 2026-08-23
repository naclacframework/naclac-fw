use naclac_lang::prelude::*;

// A PDA owned by cpi_caller, used purely as a CPI-signing authority — its
// address is what cpi_callee's `Counter.authority` gets set to, and its
// seeds are what `authorized_increment_signed` signs the cross-program
// call with. Exercises the real "signed CPI with PDA seeds" path: a PDA
// belonging to *this* program authorizing an action in *another* program.
#[component]
pub struct CallerAuthority {
    pub bump: u8,
}
