//! Permanent regression guard for the zero-copy/Borsh routing fix in
//! naclac-macros: `#[component]`/`#[event]`/`#[defined_type]`/
//! `#[instruction_args]`/`#[derive(Accounts)]` all emit BOTH representations
//! behind a real `#[cfg(...)]` in their output, since a proc-macro has no
//! reliable way to see the calling crate's own activated Cargo features
//! (see macros/src/component.rs's doc comment for the full story).
//!
//! Unlike the proc-macro itself, a `#[test]`/`const _` in THIS crate's own
//! compilation sees `cfg!(feature = "...")` reliably — it's the same crate
//! checking its own real, independently-resolved Cargo features, not a
//! proc-macro guessing from outside. So asserting a trait bound here, gated
//! on this crate's own real cfg, is a genuine end-to-end check that the
//! macros' emitted `#[cfg(...)]` actually tracks reality, not a restatement
//! of their own logic.
//!
//! This crate builds with `bytemuck` (default feature) on the pinocchio
//! runtime, so every check below expects the zero-copy shape.

use crate::{DynamicArgs, InnerThing, Thing, ThingEvent};
use std::println;

#[cfg(feature = "bytemuck")]
const _: fn() = || {
    fn assert_pod<T: naclac_lang::prelude::bytemuck::Pod>() {}
    assert_pod::<Thing>();
    assert_pod::<InnerThing>();

    // A zero-copy `#[event]` is a packed plain-data struct, so it is `Pod`;
    // the Borsh representation never derives that.
    assert_pod::<ThingEvent>();

    // `DynamicArgs` has a `ZcVec<u8>` field — never Pod — so its zero-copy
    // signal is the `NaclacArgs` deserialization impl instead.
    fn assert_naclac_args<'a, T: naclac_lang::prelude::NaclacArgs<'a>>() {}
    assert_naclac_args::<DynamicArgs>();
};

#[cfg(feature = "borsh")]
const _: fn() = || {
    fn assert_borsh<T: naclac_lang::prelude::BorshSerialize>() {}
    assert_borsh::<Thing>();
    assert_borsh::<ThingEvent>();
    assert_borsh::<InnerThing>();
    assert_borsh::<DynamicArgs>();
};

#[test]
fn reports_actual_representation() {
    println!(
        "representation_routing: solana={} bytemuck={} borsh={} -> zero-copy",
        cfg!(feature = "solana"),
        cfg!(feature = "bytemuck"),
        cfg!(feature = "borsh"),
    );
}

#[cfg(feature = "idl-build")]
#[test]
fn dumps_real_idl() {
    crate::__naclac_print_idl();
}
