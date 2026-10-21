//! Permanent regression guard for the zero-copy/Borsh routing fix in
//! naclac-macros — see `representation_routing`'s copy of this file for the
//! full explanation of why this check is meaningful and not circular.
//!
//! This crate builds with `solana` and `borsh` active (its own `default`) —
//! every check below expects the real Borsh shape.

use crate::{DoThing, DynamicArgs, InnerThing, Thing, ThingEvent};

#[cfg(feature = "bytemuck")]
const _: fn() = || {
    fn assert_pod<T: naclac_lang::prelude::bytemuck::Pod>() {}
    assert_pod::<Thing>();
    assert_pod::<InnerThing>();

    fn assert_pod_wire<E, W: naclac_lang::prelude::bytemuck::Pod>(_: fn(&E) -> W) {}
    assert_pod_wire(ThingEvent::__naclac_wire);

    fn assert_naclac_args<T: naclac_lang::prelude::NaclacArgs>() {}
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

#[cfg(feature = "solana")]
const _: fn() = || {
    fn assert_loadable<'a, T: naclac_lang::prelude::LoadableAccounts<'a>>() {}
    assert_loadable::<DoThing>();
};

#[test]
fn reports_actual_representation() {
    println!(
        "representation_routing_borsh: solana={} bytemuck={} borsh={} -> Borsh",
        cfg!(feature = "solana"),
        cfg!(feature = "bytemuck"),
        cfg!(feature = "borsh"),
    );
}
