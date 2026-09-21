//! Permanent regression guard for the zero-copy/Borsh routing fix in
//! naclac-macros — see `representation_routing`'s copy of this file for the
//! full explanation of why this check is meaningful and not circular.
//!
//! This crate builds with `borsh` active (its own `default`) and no
//! `pinocchio` — every check below expects the real Borsh shape. This is
//! the one combination that was silently broken before the fix: the old
//! `caller_has_feature`-based detection could never see `borsh` was active,
//! so every one of these types would previously have compiled with the
//! zero-copy shape instead.

use representation_routing_borsh::{DoThing, DynamicArgs, InnerThing, Thing, ThingEvent};

#[cfg(any(feature = "pinocchio", not(feature = "borsh")))]
const _: fn() = || {
    fn assert_pod<T: naclac_lang::prelude::bytemuck::Pod>() {}
    assert_pod::<Thing>();
    assert_pod::<ThingEvent>();
    assert_pod::<InnerThing>();

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

#[cfg(not(feature = "pinocchio"))]
const _: fn() = || {
    fn assert_loadable<'a, T: naclac_lang::prelude::LoadableAccounts<'a>>() {}
    assert_loadable::<DoThing>();
};

#[test]
fn reports_actual_representation() {
    println!(
        "representation_routing_borsh: pinocchio={} borsh={} -> Borsh",
        cfg!(feature = "pinocchio"),
        cfg!(feature = "borsh"),
    );
}
