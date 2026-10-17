//! `FixedVec<T, N>`: a list of at most `N` plain-data values stored inline as a
//! two-byte little-endian length followed by all `N` slots.
//!
//! The type is `#[repr(C, packed)]`: alignment 1, no padding, size
//! `2 + N * size_of::<T>()`, and the same layout whatever `T` is. Because a
//! reference to a field of a packed struct needs the field's own alignment,
//! elements are read and written by value (`get`, `set`, `iter`, `pop`, ...).
//! For elements of alignment 1 ([`Unaligned`]) the list also derefs to
//! `&[T]`.
//!
//! Every accessor treats a stored length above `N` as `N` and never touches a
//! slot at or past the length. Operations that shorten the list zero the freed
//! slots, so a list built or changed only through this type, in an account that
//! started zeroed, has the same bytes as any other list with the same contents.
//! Changing `N` changes the layout; grow an existing account with `realloc`.

use {
    crate::{error::PodError, unaligned::Unaligned},
    bytemuck::{Pod, Zeroable},
    core::{
        fmt,
        mem::size_of,
        ops::{Deref, DerefMut},
    },
};

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct FixedVec<T: Pod, const N: usize> {
    len: [u8; 2],
    data: [T; N],
}

// SAFETY: `repr(C, packed)` over a byte array and a `Pod` array has no
// padding, and every byte pattern is a valid value because the accessors clamp
// the stored length to `N`.
unsafe impl<T: Pod, const N: usize> Pod for FixedVec<T, N> {}
// SAFETY: all-zero bytes are an empty list.
unsafe impl<T: Pod, const N: usize> Zeroable for FixedVec<T, N> {}
// SAFETY: `repr(packed)` gives alignment 1 whatever `T` is.
unsafe impl<T: Pod, const N: usize> Unaligned for FixedVec<T, N> {}

impl<T: Pod, const N: usize> FixedVec<T, N> {
    const LAYOUT: () = {
        assert!(
            N <= u16::MAX as usize,
            "FixedVec capacity exceeds what its two-byte length can store"
        );
        assert!(
            size_of::<Self>() == 2 + N * size_of::<T>(),
            "FixedVec has padding"
        );
        assert!(core::mem::align_of::<Self>() == 1, "FixedVec is not packed");
    };

    #[inline(always)]
    fn data_ptr(&self) -> *const T {
        core::ptr::addr_of!(self.data).cast::<T>()
    }

    #[inline(always)]
    fn data_mut_ptr(&mut self) -> *mut T {
        core::ptr::addr_of_mut!(self.data).cast::<T>()
    }

    /// `index` must be below `N`.
    #[inline(always)]
    fn read(&self, index: usize) -> T {
        debug_assert!(index < N);
        // SAFETY: the callers pass `index < len() <= N`, so the slot lies
        // inside `data`; `T: Pod` makes any bytes valid; the read is
        // unaligned.
        unsafe { self.data_ptr().add(index).read_unaligned() }
    }

    /// `index` must be below `N`.
    #[inline(always)]
    fn write(&mut self, index: usize, value: T) {
        debug_assert!(index < N);
        // SAFETY: as in `read`, and the write is unaligned.
        unsafe { self.data_mut_ptr().add(index).write_unaligned(value) }
    }

    /// Zeroes slots `from..to`; `from` must not exceed `to`, and `to` must not
    /// exceed `N`.
    #[inline(always)]
    fn zero(&mut self, from: usize, to: usize) {
        debug_assert!(from <= to && to <= N);
        let count = to.saturating_sub(from) * size_of::<T>();
        // SAFETY: the byte range lies inside `data`, and `u8` has alignment 1.
        unsafe {
            core::ptr::write_bytes(
                self.data_mut_ptr().cast::<u8>().add(from * size_of::<T>()),
                0,
                count,
            );
        }
    }

    /// Moves slots `from..from + count` to start at `to`, overlap allowed. Both
    /// ranges must lie inside `0..N`.
    #[inline(always)]
    fn shift(&mut self, from: usize, to: usize, count: usize) {
        debug_assert!(from + count <= N && to + count <= N);
        // SAFETY: both byte ranges lie inside `data`, `u8` has alignment 1,
        // and `copy` allows overlap.
        unsafe {
            let base = self.data_mut_ptr().cast::<u8>();
            core::ptr::copy(
                base.add(from * size_of::<T>()),
                base.add(to * size_of::<T>()),
                count * size_of::<T>(),
            );
        }
    }

    #[inline(always)]
    fn set_len(&mut self, len: usize) {
        self.len = u16::try_from(len).unwrap_or(u16::MAX).to_le_bytes();
    }

    /// An empty list.
    #[inline(always)]
    pub fn new() -> Self {
        let () = Self::LAYOUT;
        Self::zeroed()
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// The number of elements; a stored length above `N` reads as `N`.
    #[inline(always)]
    pub fn len(&self) -> usize {
        let () = Self::LAYOUT;
        let stored = usize::from(u16::from_le_bytes(self.len));
        if stored > N {
            N
        } else {
            stored
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.len() == N
    }

    /// The element at `index`, or `None` at or past the length.
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(self.read(index))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn first(&self) -> Option<T> {
        self.get(0)
    }

    #[inline(always)]
    pub fn last(&self) -> Option<T> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    /// Replaces the element at `index`. False, and no change, at or past the
    /// length.
    #[inline(always)]
    pub fn set(&mut self, index: usize, value: T) -> bool {
        if index < self.len() {
            self.write(index, value);
            true
        } else {
            false
        }
    }

    /// Appends `value`, or fails with `CapacityExceeded` when the list is full.
    pub fn try_push(&mut self, value: T) -> Result<(), PodError> {
        let len = self.len();
        if len >= N {
            return Err(PodError::CapacityExceeded);
        }
        self.write(len, value);
        self.set_len(len + 1);
        Ok(())
    }

    /// Inserts `value` at `index`, shifting the following elements up. Fails
    /// with `CapacityExceeded` when the list is full or `index` is past the
    /// length.
    pub fn try_insert(&mut self, index: usize, value: T) -> Result<(), PodError> {
        let len = self.len();
        if len >= N || index > len {
            return Err(PodError::CapacityExceeded);
        }
        self.shift(index, index + 1, len - index);
        self.write(index, value);
        self.set_len(len + 1);
        Ok(())
    }

    /// Removes and returns the last element, zeroing its slot.
    pub fn pop(&mut self) -> Option<T> {
        let len = self.len();
        let last = len.checked_sub(1)?;
        let value = self.read(last);
        self.zero(last, len);
        self.set_len(last);
        Some(value)
    }

    /// Shortens the list to `new_len`, zeroing the removed slots. Does nothing
    /// when `new_len` is not below the length.
    pub fn truncate(&mut self, new_len: usize) {
        let len = self.len();
        if new_len >= len {
            return;
        }
        self.zero(new_len, len);
        self.set_len(new_len);
    }

    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Removes element `index` and shifts the following ones down. `None` at
    /// or past the length.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let len = self.len();
        if index >= len {
            return None;
        }
        let value = self.read(index);
        self.shift(index + 1, index, len - index - 1);
        self.zero(len - 1, len);
        self.set_len(len - 1);
        Some(value)
    }

    /// Removes element `index` and puts the last element in its place. `None`
    /// at or past the length.
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        let len = self.len();
        if index >= len {
            return None;
        }
        let value = self.read(index);
        let last = self.read(len - 1);
        self.write(index, last);
        self.zero(len - 1, len);
        self.set_len(len - 1);
        Some(value)
    }

    /// Keeps the elements for which `keep` is true, in order, and zeroes the
    /// slots that free up.
    pub fn retain(&mut self, mut keep: impl FnMut(T) -> bool) {
        let len = self.len();
        let mut kept = 0;
        for index in 0..len {
            let value = self.read(index);
            if keep(value) {
                if kept != index {
                    self.write(kept, value);
                }
                kept += 1;
            }
        }
        self.zero(kept, len);
        self.set_len(kept);
    }

    /// Replaces the contents with `values`, or fails with `CapacityExceeded`
    /// and leaves the list unchanged when there are more than `N`.
    pub fn try_set_from_slice(&mut self, values: &[T]) -> Result<(), PodError> {
        if values.len() > N {
            return Err(PodError::CapacityExceeded);
        }
        let old_len = self.len();
        for (index, value) in values.iter().enumerate() {
            self.write(index, *value);
        }
        if values.len() < old_len {
            self.zero(values.len(), old_len);
        }
        self.set_len(values.len());
        Ok(())
    }

    /// Appends `values`, or fails with `CapacityExceeded` and leaves the list
    /// unchanged when they do not all fit.
    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), PodError> {
        let len = self.len();
        let new_len = match len.checked_add(values.len()) {
            Some(total) if total <= N => total,
            _ => return Err(PodError::CapacityExceeded),
        };
        for (offset, value) in values.iter().enumerate() {
            self.write(len + offset, *value);
        }
        self.set_len(new_len);
        Ok(())
    }

    /// Copies the elements into `out`, which must have exactly the list's
    /// length; false, and nothing copied, otherwise.
    pub fn copy_to_slice(&self, out: &mut [T]) -> bool {
        if out.len() != self.len() {
            return false;
        }
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.read(index);
        }
        true
    }

    /// Iterates the elements by value.
    pub fn iter(&self) -> Iter<'_, T, N> {
        Iter {
            list: self,
            next: 0,
            end: self.len(),
        }
    }
}

impl<T: Pod + PartialEq, const N: usize> FixedVec<T, N> {
    pub fn contains(&self, value: &T) -> bool {
        self.iter().any(|element| element == *value)
    }
}

impl<T: Unaligned, const N: usize> FixedVec<T, N> {
    /// The elements as a slice; only for elements of alignment 1.
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: `T` has alignment 1, so any address is valid for `&[T]`; the
        // `len()` slots lie inside `data` and hold initialized `Pod` values.
        unsafe { core::slice::from_raw_parts(self.data_ptr(), self.len()) }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len();
        // SAFETY: as in `as_slice`, and `&mut self` is exclusive.
        unsafe { core::slice::from_raw_parts_mut(self.data_mut_ptr(), len) }
    }
}

impl<T: Unaligned, const N: usize> Deref for FixedVec<T, N> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Unaligned, const N: usize> DerefMut for FixedVec<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

/// By-value iterator over a [`FixedVec`].
pub struct Iter<'a, T: Pod, const N: usize> {
    list: &'a FixedVec<T, N>,
    next: usize,
    end: usize,
}

impl<T: Pod, const N: usize> Iterator for Iter<'_, T, N> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<T> {
        if self.next >= self.end {
            return None;
        }
        let value = self.list.read(self.next);
        self.next += 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.next;
        (remaining, Some(remaining))
    }
}

impl<T: Pod, const N: usize> DoubleEndedIterator for Iter<'_, T, N> {
    fn next_back(&mut self) -> Option<T> {
        if self.next >= self.end {
            return None;
        }
        self.end -= 1;
        Some(self.list.read(self.end))
    }
}

impl<T: Pod, const N: usize> ExactSizeIterator for Iter<'_, T, N> {}

impl<'a, T: Pod, const N: usize> IntoIterator for &'a FixedVec<T, N> {
    type Item = T;
    type IntoIter = Iter<'a, T, N>;

    fn into_iter(self) -> Iter<'a, T, N> {
        self.iter()
    }
}

impl<T: Pod, const N: usize> Default for FixedVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Pod + PartialEq, const N: usize> PartialEq for FixedVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl<T: Pod + Eq, const N: usize> Eq for FixedVec<T, N> {}

impl<T: Pod + fmt::Debug, const N: usize> fmt::Debug for FixedVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Debug, Pod, Zeroable)]
    #[repr(C, packed)]
    struct Pair {
        wide: u64,
        narrow: u16,
    }

    fn pair(wide: u64, narrow: u16) -> Pair {
        Pair { wide, narrow }
    }

    #[test]
    fn layout_is_a_two_byte_length_then_every_slot_with_no_padding() {
        assert_eq!(size_of::<FixedVec<u8, 5>>(), 2 + 5);
        assert_eq!(size_of::<FixedVec<u16, 3>>(), 2 + 6);
        assert_eq!(size_of::<FixedVec<u32, 3>>(), 2 + 12);
        assert_eq!(size_of::<FixedVec<u64, 3>>(), 2 + 24);
        assert_eq!(size_of::<FixedVec<u128, 2>>(), 2 + 32);
        assert_eq!(size_of::<FixedVec<[u8; 32], 4>>(), 2 + 128);
        assert_eq!(size_of::<FixedVec<Pair, 2>>(), 2 + 20);
        assert_eq!(size_of::<FixedVec<u64, 0>>(), 2);
        assert_eq!(size_of::<FixedVec<u8, 65535>>(), 2 + 65535);
        assert_eq!(core::mem::align_of::<FixedVec<u128, 2>>(), 1);
        assert_eq!(core::mem::align_of::<FixedVec<Pair, 2>>(), 1);
    }

    #[test]
    fn bytes_are_a_little_endian_length_then_the_slots() {
        let mut list = FixedVec::<u32, 3>::new();
        list.try_push(0x0A0B_0C0D)
            .unwrap_or_else(|_| panic!("push"));
        list.try_push(1).unwrap_or_else(|_| panic!("push"));
        assert_eq!(
            bytemuck::bytes_of(&list),
            &[2, 0, 0x0D, 0x0C, 0x0B, 0x0A, 1, 0, 0, 0, 0, 0, 0, 0][..]
        );
    }

    #[test]
    fn push_pop_get_set_and_capacity() {
        let mut list = FixedVec::<u32, 2>::new();
        assert!(list.is_empty());
        assert_eq!(list.capacity(), 2);
        list.try_push(7).unwrap_or_else(|_| panic!("push"));
        list.try_push(9).unwrap_or_else(|_| panic!("push"));
        assert!(list.is_full());
        assert!(list.try_push(1) == Err(PodError::CapacityExceeded));
        assert_eq!(list.get(0), Some(7));
        assert_eq!(list.get(1), Some(9));
        assert_eq!(list.get(2), None);
        assert_eq!(list.first(), Some(7));
        assert_eq!(list.last(), Some(9));
        assert!(list.set(1, 10));
        assert!(!list.set(2, 11));
        assert_eq!(list.pop(), Some(10));
        assert_eq!(list.pop(), Some(7));
        assert_eq!(list.pop(), None);
        assert_eq!(list.last(), None);
        assert!(bytemuck::bytes_of(&list).iter().all(|b| *b == 0));
    }

    #[test]
    fn freed_slots_are_zeroed_by_every_shortening_operation() {
        let all_zero_after = |list: &FixedVec<u16, 6>, from_slot: usize| {
            bytemuck::bytes_of(list)[2 + from_slot * 2..]
                .iter()
                .all(|b| *b == 0)
        };
        let full = || {
            let mut list = FixedVec::<u16, 6>::new();
            list.try_set_from_slice(&[0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD, 0xEEEE, 0xFFFF])
                .unwrap_or_else(|_| panic!("set"));
            list
        };

        let mut list = full();
        list.truncate(2);
        assert!(all_zero_after(&list, 2));

        let mut list = full();
        list.clear();
        assert!(all_zero_after(&list, 0));

        let mut list = full();
        list.pop();
        assert!(all_zero_after(&list, 5));

        let mut list = full();
        list.remove(1);
        assert!(all_zero_after(&list, 5));

        let mut list = full();
        list.swap_remove(1);
        assert!(all_zero_after(&list, 5));

        let mut list = full();
        list.retain(|v| v % 2 == 0);
        assert!(all_zero_after(&list, list.len()));

        let mut list = full();
        list.try_set_from_slice(&[1, 2])
            .unwrap_or_else(|_| panic!("set"));
        assert!(all_zero_after(&list, 2));
    }

    #[test]
    fn equal_lists_have_equal_bytes() {
        let mut a = FixedVec::<u16, 4>::new();
        a.try_set_from_slice(&[1, 2, 3, 4])
            .unwrap_or_else(|_| panic!("set"));
        a.truncate(1);
        let mut b = FixedVec::<u16, 4>::new();
        b.try_push(1).unwrap_or_else(|_| panic!("push"));
        assert_eq!(bytemuck::bytes_of(&a), bytemuck::bytes_of(&b));
        assert!(a == b);
    }

    #[test]
    fn remove_swap_remove_insert_and_retain() {
        let mut list = FixedVec::<u8, 6>::new();
        list.try_set_from_slice(&[1, 2, 3, 4])
            .unwrap_or_else(|_| panic!("set"));
        assert_eq!(list.remove(1), Some(2));
        assert_eq!(list.as_slice(), &[1, 3, 4]);
        assert_eq!(list.swap_remove(0), Some(1));
        assert_eq!(list.as_slice(), &[4, 3]);
        assert_eq!(list.remove(5), None);
        assert_eq!(list.swap_remove(2), None);

        list.try_insert(0, 9).unwrap_or_else(|_| panic!("insert"));
        list.try_insert(3, 8).unwrap_or_else(|_| panic!("insert"));
        list.try_insert(2, 7).unwrap_or_else(|_| panic!("insert"));
        assert_eq!(list.as_slice(), &[9, 4, 7, 3, 8]);
        assert!(list.try_insert(9, 1) == Err(PodError::CapacityExceeded));
        list.try_push(0).unwrap_or_else(|_| panic!("push"));
        assert!(list.try_insert(0, 1) == Err(PodError::CapacityExceeded));

        list.retain(|v| v >= 5);
        assert_eq!(list.as_slice(), &[9, 7, 8]);
    }

    #[test]
    fn multi_byte_and_struct_elements_round_trip_at_an_odd_address() {
        let mut buffer = [0u8; 40];
        let list: &mut FixedVec<Pair, 3> = bytemuck::from_bytes_mut(&mut buffer[1..1 + 2 + 30]);
        list.try_push(pair(0x1122_3344_5566_7788, 9))
            .unwrap_or_else(|_| panic!("push"));
        list.try_push(pair(1, 2)).unwrap_or_else(|_| panic!("push"));
        assert_eq!(list.get(0), Some(pair(0x1122_3344_5566_7788, 9)));
        assert_eq!(list.iter().collect::<std::vec::Vec<_>>().len(), 2);
        assert!(list.contains(&pair(1, 2)));
        assert!(!list.contains(&pair(1, 3)));
        assert_eq!(buffer[1], 2);
        assert_eq!(&buffer[3..11], &0x1122_3344_5566_7788u64.to_le_bytes());
    }

    #[test]
    fn a_stored_length_above_capacity_is_clamped_everywhere() {
        let mut bytes = [0u8; 2 + 3];
        bytes[0] = 0xFF;
        bytes[1] = 0xFF;
        bytes[2] = 1;
        bytes[3] = 2;
        bytes[4] = 3;
        let list: &mut FixedVec<u8, 3> = bytemuck::from_bytes_mut(&mut bytes);
        assert_eq!(list.len(), 3);
        assert!(list.is_full());
        assert_eq!(list.as_slice(), &[1, 2, 3]);
        assert_eq!(list.get(3), None);
        assert!(list.try_push(4) == Err(PodError::CapacityExceeded));
        assert_eq!(list.iter().count(), 3);
        assert_eq!(list.pop(), Some(3));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn failed_set_and_extend_leave_the_list_unchanged() {
        let mut list = FixedVec::<u8, 3>::new();
        list.try_set_from_slice(&[1, 2])
            .unwrap_or_else(|_| panic!("set"));
        let before = list;
        assert!(list.try_set_from_slice(&[1, 2, 3, 4]) == Err(PodError::CapacityExceeded));
        assert!(list.try_extend_from_slice(&[9, 9]) == Err(PodError::CapacityExceeded));
        assert_eq!(bytemuck::bytes_of(&list), bytemuck::bytes_of(&before));
        list.try_extend_from_slice(&[9])
            .unwrap_or_else(|_| panic!("extend"));
        assert_eq!(list.as_slice(), &[1, 2, 9]);
    }

    #[test]
    fn iterator_is_exact_size_and_double_ended() {
        let mut list = FixedVec::<u32, 5>::new();
        list.try_set_from_slice(&[10, 20, 30, 40])
            .unwrap_or_else(|_| panic!("set"));
        let mut iter = list.iter();
        assert_eq!(iter.len(), 4);
        assert_eq!(iter.next(), Some(10));
        assert_eq!(iter.next_back(), Some(40));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next_back(), Some(30));
        assert_eq!(iter.next(), Some(20));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        let mut sum = 0;
        for value in &list {
            sum += value;
        }
        assert_eq!(sum, 100);
    }

    #[test]
    fn copy_to_slice_needs_the_exact_length() {
        let mut list = FixedVec::<u16, 4>::new();
        list.try_set_from_slice(&[5, 6, 7])
            .unwrap_or_else(|_| panic!("set"));
        let mut out = [0u16; 3];
        assert!(list.copy_to_slice(&mut out));
        assert_eq!(out, [5, 6, 7]);
        let mut wrong = [0u16; 2];
        assert!(!list.copy_to_slice(&mut wrong));
        assert_eq!(wrong, [0, 0]);
    }

    #[test]
    fn slices_of_alignment_one_elements_deref_and_mutate_in_place() {
        let mut list = FixedVec::<[u8; 4], 3>::new();
        list.try_push([1, 2, 3, 4])
            .unwrap_or_else(|_| panic!("push"));
        list.try_push([5, 6, 7, 8])
            .unwrap_or_else(|_| panic!("push"));
        assert_eq!(list.len(), 2);
        assert_eq!(list[1], [5, 6, 7, 8]);
        list[0][0] = 99;
        assert_eq!(list.get(0), Some([99, 2, 3, 4]));
        assert_eq!(list.iter().count(), 2);
        assert!(list.as_slice().contains(&[5, 6, 7, 8]));
    }

    #[test]
    fn a_zero_capacity_list_is_always_empty_and_full() {
        let mut list = FixedVec::<u64, 0>::new();
        assert!(list.is_empty() && list.is_full());
        assert!(list.try_push(1) == Err(PodError::CapacityExceeded));
        assert_eq!(list.pop(), None);
        assert_eq!(list.iter().count(), 0);
        assert_eq!(bytemuck::bytes_of(&list), &[0u8, 0][..]);
    }

    #[test]
    fn debug_lists_the_elements() {
        let mut list = FixedVec::<u8, 4>::new();
        list.try_set_from_slice(&[1, 2])
            .unwrap_or_else(|_| panic!("set"));
        assert_eq!(std::format!("{list:?}"), "[1, 2]");
    }
}
