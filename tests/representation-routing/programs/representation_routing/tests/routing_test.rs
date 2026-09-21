//! Permanent regression guard for the zero-copy/Borsh routing fix in
//! naclac-macros: `#[component]`/`#[event]`/`#[defined_type]`/
//! `#[instruction_args]`/`#[derive(Accounts)]` all emit BOTH representations
//! behind a real `#[cfg(...)]` in their output, since a proc-macro has no
//! reliable way to see the calling crate's own activated Cargo features
//! (see naclac-macros/src/component.rs's doc comment for the full story).
//!
//! Unlike the proc-macro itself, a `#[test]`/`const _` in THIS crate's own
//! compilation sees `cfg!(feature = "...")` reliably — it's the same crate
//! checking its own real, independently-resolved Cargo features, not a
//! proc-macro guessing from outside. So asserting a trait bound here, gated
//! on this crate's own real cfg, is a genuine end-to-end check that the
//! macros' emitted `#[cfg(...)]` actually tracks reality, not a restatement
//! of their own logic.
//!
//! This crate builds with `pinocchio` (default feature) — always zero-copy,
//! regardless of `borsh` — so every check below expects the zero-copy shape.

use representation_routing::{DoThing, DynamicArgs, InnerThing, Thing, ThingEvent};

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
const _: fn() = || {
    fn assert_pod<T: naclac_lang::prelude::bytemuck::Pod>() {}
    assert_pod::<Thing>();
    assert_pod::<ThingEvent>();
    assert_pod::<InnerThing>();

    // `DynamicArgs` has a `ZcVec<u8>` field — never Pod — so its zero-copy
    // signal is the `NaclacArgs` deserialization impl instead.
    fn assert_naclac_args<T: naclac_lang::prelude::NaclacArgs>() {}
    assert_naclac_args::<DynamicArgs>();
};

#[cfg(all(not(feature = "pinocchio"), feature = "borsh"))]
const _: fn() = || {
    fn assert_borsh<T: naclac_lang::prelude::BorshSerialize>() {}
    assert_borsh::<Thing>();
    assert_borsh::<ThingEvent>();
    assert_borsh::<InnerThing>();
    assert_borsh::<DynamicArgs>();
};

// `#[derive(Accounts)]` routing: a non-pinocchio crate implements the real
// `LoadableAccounts` trait for its Accounts struct; a pinocchio crate gets a
// plain inherent `load_and_validate` instead (accounts.rs). Combined with
// the Pod/Borsh checks above — driven by the exact same zero_copy_cfg/
// borsh_cfg formula, in the same crate's own compilation — this is real
// evidence `DoThing`'s own internal representation choice tracked the same
// reality `Thing`/`ThingEvent`/`InnerThing`/`DynamicArgs` did.
#[cfg(not(feature = "pinocchio"))]
const _: fn() = || {
    fn assert_loadable<'a, T: naclac_lang::prelude::LoadableAccounts<'a>>() {}
    assert_loadable::<DoThing>();
};

#[test]
fn reports_actual_representation() {
    println!(
        "representation_routing: pinocchio={} borsh={} -> zero-copy (pinocchio always wins)",
        cfg!(feature = "pinocchio"),
        cfg!(feature = "borsh"),
    );
}

#[cfg(feature = "idl-build")]
#[test]
fn dumps_real_idl() {
    println!("{}", representation_routing::__NACLAC_IDL_JSON);
}
