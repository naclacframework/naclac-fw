//! The layout-dependent pieces of an Accounts struct's expansion, computed
//! once per layout and spliced into the shared impls.

use {
    super::{
        field_loaders,
        ix_args::InstructionArgs,
        layout::Layout,
        stack_guard::size_guard_consts,
        teardown::{mut_bump_writeback, save_logic},
    },
    crate::accounts::field::parser,
    proc_macro2::TokenStream,
};

pub(super) struct Variant {
    pub(super) ix_deserializer: TokenStream,
    pub(super) field_loaders: Vec<TokenStream>,
    pub(super) size_guard_consts: TokenStream,
    pub(super) save_logic: Vec<TokenStream>,
    pub(super) mut_bump_writeback: Vec<TokenStream>,
}

impl Variant {
    pub(super) fn build(
        layout: Layout,
        fields: &[parser::ParsedField],
        args: &InstructionArgs,
        load_order: &[usize],
    ) -> Self {
        let cfg = layout.cfg();
        Variant {
            ix_deserializer: args.deserializer(layout),
            field_loaders: field_loaders::build(fields, load_order),
            size_guard_consts: size_guard_consts(fields, layout, &cfg),
            save_logic: save_logic(fields, layout, &cfg),
            mut_bump_writeback: mut_bump_writeback(fields, &cfg),
        }
    }
}

pub(super) struct Variants {
    pub(super) zero_copy: Variant,
    pub(super) borsh: Variant,
    pub(super) wincode: Variant,
}
