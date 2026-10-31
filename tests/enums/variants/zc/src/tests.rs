//! Host-side checks of the macro-generated `ZcEnum` support on the real `State` account.

use crate::state::State;
use crate::types::{Action, Color, Level, Signed, Wide};
use core::mem::{align_of, offset_of, size_of};
use naclac_lang::prelude::{bytemuck, vec, Address, Opt, ZcEnum};

fn fresh() -> State {
    bytemuck::Zeroable::zeroed()
}

fn address(byte: u8) -> Address {
    bytemuck::cast::<[u8; 32], Address>([byte; 32])
}

#[test]
fn a_stored_enum_is_its_tag_plus_the_largest_variant_with_alignment_one() {
    assert_eq!(size_of::<ZcEnum<Color>>(), 1);
    assert_eq!(size_of::<ZcEnum<Action>>(), 1 + 8 + 32);
    assert_eq!(size_of::<ZcEnum<Level>>(), 2);
    assert_eq!(size_of::<ZcEnum<Signed>>(), 1);
    assert_eq!(size_of::<ZcEnum<Wide>>(), 2 + 8);
    assert_eq!(align_of::<ZcEnum<Color>>(), 1);
    assert_eq!(align_of::<ZcEnum<Action>>(), 1);
    assert_eq!(align_of::<ZcEnum<Wide>>(), 1);
}

#[test]
fn the_account_is_packed_with_no_padding() {
    assert_eq!(align_of::<State>(), 1);
    assert_eq!(size_of::<State>(), 1 + 32 + 1 + 41 + 4 + 2 + 17 + 2 + 1 + 10 + 1);
    assert_eq!(offset_of!(State, admin), 1);
    assert_eq!(offset_of!(State, color), 33);
    assert_eq!(offset_of!(State, action), 34);
    assert_eq!(offset_of!(State, history), 75);
}

#[test]
fn a_new_account_reads_as_the_variants_with_tag_zero() {
    let state = fresh();
    assert!(matches!(state.color.get(), Ok(Color::Red)));
    assert!(matches!(state.action.get(), Ok(Action::Pause)));
    assert!(matches!(state.limits.mode.get(), Ok(Color::Red)));
    for slot in state.history {
        assert!(matches!(slot.get(), Ok(Color::Red)));
    }
    assert!(state.maybe_color.is_none());
}

#[test]
fn every_enum_field_round_trips() {
    let mut state = fresh();

    state.color.set(Color::Blue);
    assert!(matches!(state.color.get(), Ok(Color::Blue)));

    state.action.set(Action::Transfer { amount: 123_456_789, to: address(7) });
    match state.action.get() {
        Ok(Action::Transfer { amount, to }) => {
            assert_eq!(amount, 123_456_789);
            assert_eq!(bytemuck::bytes_of(&to), &[7u8; 32]);
        }
        _ => panic!("wrong variant"),
    }

    state.action.set(Action::Paint(Color::Green));
    assert!(matches!(state.action.get(), Ok(Action::Paint(Color::Green))));

    state.history[2].set(Color::Blue);
    assert!(matches!(state.history[2].get(), Ok(Color::Blue)));
    assert!(matches!(state.history[1].get(), Ok(Color::Red)));

    state.limits.mode.set(Color::Green);
    assert!(matches!(state.limits.mode.get(), Ok(Color::Green)));

    state.maybe_color = Opt::from(Some(ZcEnum::<Color>::new(Color::Blue)));
    assert!(state.maybe_color.is_some());
    let stored = state.maybe_color.get().expect("the option was just set");
    assert!(matches!(stored.get(), Ok(Color::Blue)));
}

#[test]
fn switching_to_a_smaller_variant_leaves_no_stale_bytes() {
    let mut state = fresh();
    state.action.set(Action::Transfer { amount: u64::MAX, to: address(0xFF) });
    state.action.set(Action::Pause);

    let start = offset_of!(State, action);
    let bytes = bytemuck::bytes_of(&state);
    assert!(bytes[start..start + size_of::<ZcEnum<Action>>()]
        .iter()
        .all(|byte| *byte == 0));
}

#[test]
fn the_tag_and_fields_land_at_their_offsets() {
    let mut state = fresh();
    state.action.set(Action::SetFee { fee_bps: 0x0102 });

    let start = offset_of!(State, action);
    let bytes = bytemuck::bytes_of(&state);
    assert_eq!(bytes[start], 1);
    assert_eq!(&bytes[start + 1..start + 3], &[0x02, 0x01]);

    state.action.set(Action::Transfer { amount: 0x0807_0605_0403_0201, to: address(9) });
    let bytes = bytemuck::bytes_of(&state);
    assert_eq!(bytes[start], 2);
    assert_eq!(&bytes[start + 1..start + 9], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(&bytes[start + 9..start + 41], &[9u8; 32]);
}

#[test]
fn invalid_bytes_are_errors_in_every_enum_field() {
    let mut state = fresh();
    let color_at = offset_of!(State, color);
    let action_at = offset_of!(State, action);
    let history_at = offset_of!(State, history) + 3;
    {
        let bytes = bytemuck::bytes_of_mut(&mut state);
        bytes[color_at] = 2;
        bytes[action_at] = 9;
        bytes[history_at] = 200;
    }
    assert!(state.color.get().is_err());
    assert!(state.action.get().is_err());
    assert!(state.history[3].get().is_err());
    assert!(state.history[0].get().is_ok());
}

#[test]
fn an_invalid_enum_inside_a_variant_is_an_error() {
    let mut state = fresh();
    let action_at = offset_of!(State, action);
    {
        let bytes = bytemuck::bytes_of_mut(&mut state);
        bytes[action_at] = 3;
        bytes[action_at + 1] = 2;
    }
    assert!(state.action.get().is_err());

    bytemuck::bytes_of_mut(&mut state)[action_at + 1] = 7;
    assert!(matches!(state.action.get(), Ok(Action::Paint(Color::Blue))));
}

#[test]
fn the_account_survives_a_byte_copy() {
    let mut state = fresh();
    state.color.set(Color::Blue);
    state.action.set(Action::Paint(Color::Green));

    let copied: State = bytemuck::pod_read_unaligned(bytemuck::bytes_of(&state));
    assert!(matches!(copied.color.get(), Ok(Color::Blue)));
    assert!(matches!(copied.action.get(), Ok(Action::Paint(Color::Green))));
}

#[test]
fn the_account_reads_the_same_at_every_address() {
    let mut state = fresh();
    state.color.set(Color::Blue);
    state.action.set(Action::SetFee { fee_bps: 77 });
    state.counter = 9;

    let bytes = bytemuck::bytes_of(&state).to_vec();
    for offset in 0..8 {
        let mut buffer = vec![0u8; offset];
        buffer.extend_from_slice(&bytes);
        let view: &State = bytemuck::from_bytes(&buffer[offset..]);
        assert!(matches!(view.color.get(), Ok(Color::Blue)));
        assert!(matches!(view.action.get(), Ok(Action::SetFee { fee_bps: 77 })));
        assert_eq!(view.counter, 9);
    }
}

#[test]
fn derived_comparison_works_on_a_value_read_back() {
    let mut state = fresh();
    state.color.set(Color::Blue);
    assert!(state.color.get().ok() == Some(Color::Blue));
    assert!(state.color.get().ok() != Some(Color::Green));
}

#[test]
fn a_u16_tag_is_stored_in_two_little_endian_bytes() {
    let mut state = fresh();
    let at = offset_of!(State, level);
    state.level.set(Level::Mid);
    assert_eq!(&bytemuck::bytes_of(&state)[at..at + 2], &300u16.to_le_bytes());
    state.level.set(Level::High);
    assert_eq!(&bytemuck::bytes_of(&state)[at..at + 2], &65_000u16.to_le_bytes());
    assert!(matches!(state.level.get(), Ok(Level::High)));
}

#[test]
fn an_enum_without_a_zero_variant_reads_as_an_error_until_it_is_set() {
    let mut state = fresh();
    assert!(state.level.get().is_err());
    state.level.set(Level::Low);
    assert!(matches!(state.level.get(), Ok(Level::Low)));
}

#[test]
fn a_negative_tag_is_stored_as_its_twos_complement_byte() {
    let mut state = fresh();
    let at = offset_of!(State, signed);
    assert!(matches!(state.signed.get(), Ok(Signed::Zero)));
    state.signed.set(Signed::Neg);
    assert_eq!(bytemuck::bytes_of(&state)[at], 0xFD);
    assert!(matches!(state.signed.get(), Ok(Signed::Neg)));
    bytemuck::bytes_of_mut(&mut state)[at] = 1;
    assert!(state.signed.get().is_err());
}

#[test]
fn a_data_enum_with_a_u16_tag_lays_out_tag_then_fields() {
    let mut state = fresh();
    let at = offset_of!(State, wide);

    state.wide.set(Wide::Small(9));
    let bytes = bytemuck::bytes_of(&state);
    assert_eq!(&bytes[at..at + 2], &1u16.to_le_bytes());
    assert_eq!(bytes[at + 2], 9);

    state.wide.set(Wide::Big { value: 0x0102_0304_0506_0708 });
    let bytes = bytemuck::bytes_of(&state);
    assert_eq!(&bytes[at..at + 2], &2u16.to_le_bytes());
    assert_eq!(&bytes[at + 2..at + 10], &0x0102_0304_0506_0708u64.to_le_bytes());

    bytemuck::bytes_of_mut(&mut state)[at..at + 2].copy_from_slice(&3u16.to_le_bytes());
    assert!(state.wide.get().is_err());
}

#[test]
fn the_default_space_covers_the_whole_account() {
    assert_eq!(State::SPACE, size_of::<State>() + 8);
}
