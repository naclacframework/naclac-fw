//! Permanent regression guard for the zero-copy/Borsh routing fix in
//! naclac-macros — see `representation_routing`'s copy of this file for the
//! full explanation of why this check is meaningful and not circular.
//!
//! This crate builds with `borsh` or `wincode` active (its own `default`) —
//! every check below expects the heap shape of that layout.

use crate::{DynamicArgs, InnerThing, Thing, ThingEvent};
use std::println;

#[cfg(feature = "bytemuck")]
const _: fn() = || {
    fn assert_pod<T: naclac_lang::prelude::bytemuck::Pod>() {}
    assert_pod::<Thing>();
    assert_pod::<InnerThing>();

    assert_pod::<ThingEvent>();

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

#[cfg(feature = "wincode")]
const _: fn() = || {
    fn assert_wincode<
        T: naclac_lang::prelude::SchemaWrite<naclac_lang::wincode_config::Config, Src = T>,
    >() {
    }
    assert_wincode::<Thing>();
    assert_wincode::<ThingEvent>();
    assert_wincode::<InnerThing>();
    assert_wincode::<DynamicArgs>();
};

#[cfg(feature = "solana")]
const _: fn() = || {
    fn assert_loadable<'a, T: naclac_lang::prelude::LoadableAccounts<'a>>() {}
    assert_loadable::<crate::DoThing>();
};

#[test]
fn reports_actual_representation() {
    println!(
        "representation_routing: solana={} bytemuck={} borsh={} wincode={} -> heap",
        cfg!(feature = "solana"),
        cfg!(feature = "bytemuck"),
        cfg!(feature = "borsh"),
        cfg!(feature = "wincode"),
    );
}
