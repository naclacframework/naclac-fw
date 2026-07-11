use naclac_lang::prelude::*;

// `bump` is a plain field here (not a framework-reserved name beyond
// convention) — the framework auto-writes the derived bump into it after
// `init` whenever the field's `seeds` constraint carries bare `bump` (no
// explicit value). See naclac-macros/src/accounts.rs's `mut_zero_copy_fields`
// handling.
#[component]
pub struct Registry {
    pub bump: u8,
    pub tag: u32,
    // Fixed-size byte array — a non-primitive field type per the IDL schema
    // (`{"array": ["u8", 4]}`, an object, not a bare primitive string).
    // Exists purely so `init_tagged_child`'s seed can depend on it, exercising
    // the client generator's "field type didn't resolve to a primitive, skip
    // the PDA helper rather than guess" path. See `init_tagged_child.rs`.
    pub label: [u8; 4],
}
