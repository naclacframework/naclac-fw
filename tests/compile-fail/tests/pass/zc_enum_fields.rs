#![allow(unexpected_cfgs)]

use naclac_lang::prelude::*;

#[defined_type]
pub enum Mode {
    Off,
    On,
}

#[defined_type]
pub struct Settings {
    pub mode: ZcEnum<Mode>,
}

#[component]
pub struct Foo {
    pub mode: ZcEnum<Mode>,
    pub modes: [ZcEnum<Mode>; 2],
    pub settings: Settings,
}

fn main() {
    let mut foo: Foo = bytemuck::Zeroable::zeroed();
    foo.mode.set(Mode::On);
    assert!(matches!(foo.mode.get(), Ok(Mode::On)));
}
