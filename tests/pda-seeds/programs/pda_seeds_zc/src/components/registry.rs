use naclac_lang::prelude::*;

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
