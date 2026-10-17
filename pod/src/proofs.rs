//! Kani proofs: each property holds for every possible input within the
//! stated sizes, not a sample. Run with `cargo kani -p naclac-pod`.

use {
    crate::{Bool, BoolField, FixedString, FixedVec, Opt, PodError},
    bytemuck::Zeroable,
};

fn list_from_any_bytes<const N: usize>(bytes: &[u8]) -> FixedVec<u8, N> {
    bytemuck::pod_read_unaligned(bytes)
}

#[kani::proof]
fn length_never_exceeds_capacity() {
    let bytes: [u8; 6] = kani::any();
    let list = list_from_any_bytes::<4>(&bytes);
    assert!(list.len() <= 4);
    assert!(list.is_full() == (list.len() == 4));
    assert!(list.is_empty() == (list.len() == 0));
}

#[kani::proof]
fn get_is_some_exactly_inside_the_length() {
    let bytes: [u8; 6] = kani::any();
    let index: usize = kani::any();
    let list = list_from_any_bytes::<4>(&bytes);
    assert!(list.get(index).is_some() == (index < list.len()));
}

#[kani::proof]
fn push_then_pop_restores_the_list() {
    let bytes: [u8; 6] = kani::any();
    let value: u8 = kani::any();
    let mut list = list_from_any_bytes::<4>(&bytes);
    let before = list.len();
    match list.try_push(value) {
        Ok(()) => {
            assert!(before < 4);
            assert!(list.len() == before + 1);
            assert!(list.last() == Some(value));
            assert!(list.pop() == Some(value));
            assert!(list.len() == before);
        }
        Err(error) => {
            assert!(error == PodError::CapacityExceeded);
            assert!(before == 4);
            assert!(list.len() == 4);
        }
    }
}

#[kani::proof]
fn pop_zeroes_the_freed_slot() {
    let bytes: [u8; 6] = kani::any();
    let mut list = list_from_any_bytes::<4>(&bytes);
    let len = list.len();
    if list.pop().is_some() {
        let after: [u8; 6] = bytemuck::cast(list);
        assert!(after[2 + len - 1] == 0);
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn truncate_zeroes_exactly_the_removed_slots() {
    let bytes: [u8; 6] = kani::any();
    let new_len: usize = kani::any();
    let mut list = list_from_any_bytes::<4>(&bytes);
    let len = list.len();
    list.truncate(new_len);
    let after: [u8; 6] = bytemuck::cast(list);
    if new_len < len {
        assert!(list.len() == new_len);
        let mut slot = new_len;
        while slot < len {
            assert!(after[2 + slot] == 0);
            slot += 1;
        }
    } else {
        assert!(list.len() == len);
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn remove_keeps_the_order_of_the_others() {
    let bytes: [u8; 2 + 3 * 2] = kani::any();
    let index: usize = kani::any();
    let mut list: FixedVec<u16, 3> = bytemuck::pod_read_unaligned(&bytes);
    let len = list.len();
    let mut model = [0u16; 3];
    let mut slot = 0;
    while slot < len {
        model[slot] = list.get(slot).unwrap_or(0);
        slot += 1;
    }
    match list.remove(index) {
        Some(removed) => {
            assert!(index < len);
            assert!(removed == model[index]);
            assert!(list.len() == len - 1);
            let mut at = 0;
            while at < len - 1 {
                let expected = if at < index { model[at] } else { model[at + 1] };
                assert!(list.get(at) == Some(expected));
                at += 1;
            }
        }
        None => assert!(index >= len),
    }
}

#[kani::proof]
fn swap_remove_fills_the_hole_with_the_last_element() {
    let bytes: [u8; 2 + 3 * 2] = kani::any();
    let index: usize = kani::any();
    let mut list: FixedVec<u16, 3> = bytemuck::pod_read_unaligned(&bytes);
    let len = list.len();
    let last = list.last();
    let removed = list.get(index);
    match list.swap_remove(index) {
        Some(value) => {
            assert!(removed == Some(value));
            assert!(list.len() == len - 1);
            if index < len - 1 {
                assert!(list.get(index) == last);
            }
        }
        None => assert!(index >= len),
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn set_and_extend_never_change_the_list_when_they_fail() {
    let bytes: [u8; 6] = kani::any();
    let extra: [u8; 3] = kani::any();
    let mut list = list_from_any_bytes::<4>(&bytes);
    let before: [u8; 6] = bytemuck::cast(list);
    if list.try_extend_from_slice(&extra).is_err() {
        let after: [u8; 6] = bytemuck::cast(list);
        assert!(after == before);
    }
    let mut list = list_from_any_bytes::<4>(&bytes);
    let too_many = [0u8; 5];
    assert!(list.try_set_from_slice(&too_many) == Err(PodError::CapacityExceeded));
    let after: [u8; 6] = bytemuck::cast(list);
    assert!(after == before);
}

#[kani::proof]
#[kani::unwind(8)]
fn string_truncate_never_ends_inside_a_character() {
    let bytes: [u8; 6] = kani::any();
    let new_len: usize = kani::any();
    let mut text: FixedString<4> = bytemuck::pod_read_unaligned(&bytes);
    let before: [u8; 4] = {
        let mut copy = [0u8; 4];
        let source = text.as_bytes();
        let mut at = 0;
        while at < source.len() {
            copy[at] = source[at];
            at += 1;
        }
        copy
    };
    let len = text.len();
    text.truncate(new_len);
    assert!(text.len() <= len);
    let kept = text.as_bytes();
    let mut at = 0;
    while at < kept.len() {
        assert!(kept[at] == before[at]);
        at += 1;
    }
    if new_len < len {
        let boundary = text.len();
        assert!(boundary <= new_len);
        if boundary < len {
            assert!(before[boundary] & 0b1100_0000 != 0b1000_0000 || boundary == 0);
        }
    }
}

#[kani::proof]
fn opt_reads_some_exactly_for_tag_one() {
    let bytes: [u8; 5] = kani::any();
    let slot: Opt<u32> = bytemuck::pod_read_unaligned(&bytes);
    assert!(slot.is_some() == (bytes[0] == 1));
    assert!(slot.get().is_some() == (bytes[0] == 1));
    assert!(slot.tag_is_valid() == (bytes[0] <= 1));
    if bytes[0] == 1 {
        assert!(slot.get() == Some(u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]])));
    }
}

#[kani::proof]
fn opt_none_and_cleared_slots_are_all_zero() {
    let value: u32 = kani::any();
    let mut slot = Opt::some(value);
    assert!(slot.get() == Some(value));
    slot.clear();
    let bytes: [u8; 5] = bytemuck::cast(slot);
    assert!(bytes == [0u8; 5]);
    assert!(Opt::<u32>::zeroed().is_none());
}

#[kani::proof]
fn bool_reads_nonzero_as_true_and_writes_canonically() {
    let byte: u8 = kani::any();
    let mut flag = Bool(byte);
    assert!(flag.get() == (byte != 0));
    assert!(bool::from(flag) == (byte != 0));
    let value: bool = kani::any();
    flag.set(value);
    assert!(flag.0 == u8::from(value));
    assert!(Bool::from(value).0 == u8::from(value));
    assert!(!!Bool(byte) == Bool(byte));
}
