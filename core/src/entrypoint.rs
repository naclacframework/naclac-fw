//! # Standard Solana backend entrypoint
//!
//! Parses the loader's serialized instruction input into the SDK's
//! `AccountInfo`s and defines the `entrypoint` symbol around a program's
//! `process_instruction`. The input layout is the loader's and must match
//! `solana_program_entrypoint::deserialize` field for field (the unit tests
//! compare the two on the same buffers). The one difference: no input can
//! make this parser panic, because a panic links the integer-formatting
//! machinery into every program.
//!
//! Every offset indexes into the buffer the loader wrote, so none of the
//! offset arithmetic can overflow `usize`.

use {
    crate::{cursor::DuplicateSlots, prelude::Vec},
    core::{
        alloc::{GlobalAlloc, Layout},
        mem::size_of,
        ptr::null_mut,
        slice::{from_raw_parts, from_raw_parts_mut},
    },
    solana_account_info::{AccountInfo, MAX_PERMITTED_DATA_INCREASE},
    solana_address::Address,
};

/// The value `entrypoint` returns when the instruction succeeded.
pub const SUCCESS: u64 = 0;

/// Start of the heap region the loader maps for every program.
pub const HEAP_START_ADDRESS: u64 = 0x300000000;
/// Length of the heap region the loader maps for every program.
pub const HEAP_LENGTH: usize = 32 * 1024;

/// The default global allocator: hands out memory downward from the end of
/// its region and never frees it, so an allocation stays valid for the rest
/// of the program's run. The first word of the region holds the position of
/// the lowest byte handed out so far (zero before the first allocation).
pub struct BumpAllocator {
    start: usize,
    len: usize,
}

impl BumpAllocator {
    /// # Safety
    ///
    /// `start..start + len` must be writable for the whole program run and
    /// used by nothing else, and must not reach the end of the address space.
    pub const unsafe fn with_fixed_address_range(start: usize, len: usize) -> Self {
        Self { start, len }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pos_ptr = self.start as *mut usize;
        let mut pos = *pos_ptr;
        if pos == 0 {
            pos = self.start + self.len;
        }
        pos = pos.saturating_sub(layout.size());
        pos &= !(layout.align().wrapping_sub(1));
        if pos < self.start + size_of::<*mut u8>() {
            return null_mut();
        }
        *pos_ptr = pos;
        pos as *mut u8
    }

    #[inline]
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

const NON_DUP_MARKER: u8 = u8::MAX;
const BPF_ALIGN_OF_U128: usize = 8;

/// The loader's per-instruction account limit
/// (`solana_transaction_context::MAX_ACCOUNTS_PER_INSTRUCTION`). Bounding the
/// count keeps the account list's allocation size provably small.
const MAX_ACCOUNTS: usize = 255;
const _: () = assert!(MAX_ACCOUNTS <= u8::MAX as usize);

/// # Safety
///
/// `input + offset` must point at a non-duplicate account record (the bytes
/// after its duplicate marker) inside the loader's input buffer, which must
/// be 8-byte aligned.
#[inline(always)]
unsafe fn deserialize_account_info<'a>(
    input: *mut u8,
    mut offset: usize,
) -> (AccountInfo<'a>, usize) {
    let is_signer = *(input.add(offset) as *const u8) != 0;
    offset = offset.wrapping_add(size_of::<u8>());

    let is_writable = *(input.add(offset) as *const u8) != 0;
    offset = offset.wrapping_add(size_of::<u8>());

    let executable = *(input.add(offset) as *const u8) != 0;
    offset = offset.wrapping_add(size_of::<u8>());

    // The 4 padding bytes record the original data length, which the
    // runtime's reallocation limit is measured against.
    let original_data_len_offset = offset;
    offset = offset.wrapping_add(size_of::<u32>());

    let key: &Address = &*(input.add(offset) as *const Address);
    offset = offset.wrapping_add(size_of::<Address>());

    let owner: &Address = &*(input.add(offset) as *const Address);
    offset = offset.wrapping_add(size_of::<Address>());

    let lamports = &mut *(input.add(offset) as *mut u64);
    offset = offset.wrapping_add(size_of::<u64>());

    let data_len = *(input.add(offset) as *const u64) as usize;
    offset = offset.wrapping_add(size_of::<u64>());

    *(input.add(original_data_len_offset) as *mut u32) = data_len as u32;

    let data = from_raw_parts_mut(input.add(offset), data_len);
    offset = offset
        .wrapping_add(data_len)
        .wrapping_add(MAX_PERMITTED_DATA_INCREASE)
        .wrapping_add(size_of::<u64>());
    offset = offset.wrapping_add(offset.wrapping_neg() & (BPF_ALIGN_OF_U128 - 1));

    (
        AccountInfo::new(
            key,
            is_signer,
            is_writable,
            lamports,
            data,
            owner,
            executable,
        ),
        offset,
    )
}

/// What [`deserialize`] returns: the program id, the accounts, the instruction
/// data and the repeats among the accounts, `None` when no real account
/// repeats.
pub type Deserialized<'a> = (
    &'a Address,
    Vec<AccountInfo<'a>>,
    &'a [u8],
    Option<DuplicateSlots>,
);

/// Parses `input` into the program id, the accounts, the instruction data and
/// the repeated accounts. Returns `None` if the account count exceeds
/// [`MAX_ACCOUNTS`] or a duplicate marker does not name an earlier account,
/// neither of which the loader produces.
///
/// A slot with a duplicate marker repeats the slot the marker names, which
/// the loader always sets to the first slot holding that account. A repeat of
/// the program's own address is an absent optional account's sentinel, not an
/// alias of a real account, so it is not reported.
///
/// # Safety
///
/// `input` must be the 8-byte-aligned input buffer the loader passes to the
/// program entrypoint, valid and exclusively owned for `'a`.
pub unsafe fn deserialize<'a>(input: *mut u8) -> Option<Deserialized<'a>> {
    let num_accounts = *(input as *const u64) as usize;
    if num_accounts > MAX_ACCOUNTS {
        return None;
    }
    let mut offset = size_of::<u64>();

    let mut duplicates: Option<DuplicateSlots> = None;
    let mut accounts: Vec<AccountInfo<'a>> = Vec::with_capacity(num_accounts);
    let slots = accounts.as_mut_ptr();
    for i in 0..num_accounts {
        let dup_info = *(input.add(offset) as *const u8);
        offset = offset.wrapping_add(size_of::<u8>());
        if dup_info == NON_DUP_MARKER {
            let (account_info, new_offset) = deserialize_account_info(input, offset);
            offset = new_offset;
            // SAFETY: `i < num_accounts <= capacity`, and slot `i` is unwritten.
            slots.add(i).write(account_info);
        } else {
            offset = offset.wrapping_add(7);
            if dup_info as usize >= i {
                // SAFETY: exactly slots `0..i` have been written. The list
                // is leaked rather than dropped: the caller fails the
                // instruction and the program's memory is discarded.
                accounts.set_len(i);
                core::mem::forget(accounts);
                return None;
            }
            // SAFETY: `dup_info < i`, so that slot is written; `i` is unwritten.
            let original = (*slots.add(dup_info as usize)).clone();
            slots.add(i).write(original);
            duplicates
                .get_or_insert_with(DuplicateSlots::default)
                .record(i as u8, dup_info);
        }
    }
    // SAFETY: all `num_accounts` slots were written above.
    accounts.set_len(num_accounts);

    let instruction_data_len = *(input.add(offset) as *const u64) as usize;
    offset = offset.wrapping_add(size_of::<u64>());
    let instruction_data = from_raw_parts(input.add(offset), instruction_data_len);
    offset = offset.wrapping_add(instruction_data_len);

    let program_id: &Address = &*(input.add(offset) as *const Address);

    if let Some(repeats) = &mut duplicates {
        for (slot, account) in accounts.iter().enumerate() {
            if repeats.involves(slot as u8) && account.key == program_id {
                repeats.forget(slot as u8);
            }
        }
        if repeats.is_empty() {
            duplicates = None;
        }
    }

    Some((program_id, accounts, instruction_data, duplicates))
}

/// Defines the program's `entrypoint` around `$process_instruction`, using
/// [`deserialize`] for the input. `$process_instruction` takes the program id,
/// the accounts, the instruction data and the repeated accounts that
/// [`deserialize`] reports. A malformed input buffer returns
/// `InvalidAccountData` without running the program. The parsed account list
/// is deliberately never dropped: the program's memory is discarded when the
/// entrypoint returns, so releasing the `Rc`s would only cost compute.
///
/// On the Solana target it also installs [`BumpAllocator`] over the loader's
/// heap region as the global allocator, unless the program crate enables its
/// own `custom-heap` feature. Panic support is not part of this macro: it
/// depends on whether the program's build includes `std` (see
/// [`crate::panic`]).
#[macro_export]
macro_rules! naclac_solana_entrypoint {
    ($process_instruction:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
            let Some((program_id, accounts, instruction_data, duplicates)) =
                (unsafe { $crate::entrypoint::deserialize(input) })
            else {
                return $crate::prelude::ProgramError::InvalidAccountData.into();
            };
            let result = match $process_instruction(
                program_id,
                &accounts,
                instruction_data,
                duplicates.as_ref(),
            ) {
                Ok(()) => $crate::entrypoint::SUCCESS,
                Err(error) => error.into(),
            };
            ::core::mem::forget(accounts);
            result
        }

        #[cfg(all(not(feature = "custom-heap"), target_os = "solana"))]
        #[global_allocator]
        static __NACLAC_HEAP: $crate::entrypoint::BumpAllocator = unsafe {
            $crate::entrypoint::BumpAllocator::with_fixed_address_range(
                $crate::entrypoint::HEAP_START_ADDRESS as usize,
                $crate::entrypoint::HEAP_LENGTH,
            )
        };
    };
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloc::{rc::Rc, vec},
    };

    enum Record {
        Account {
            key: u8,
            owner: u8,
            signer: bool,
            writable: bool,
            executable: bool,
            lamports: u64,
            data: &'static [u8],
        },
        Duplicate(u8),
    }

    fn account(
        key: u8,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: &'static [u8],
    ) -> Record {
        Record::Account {
            key,
            owner: 0xA0u8.wrapping_add(key),
            signer,
            writable,
            executable: key % 2 == 1,
            lamports,
            data,
        }
    }

    fn serialize(records: &[Record], instruction_data: &[u8], program_id: u8) -> Vec<u64> {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&(records.len() as u64).to_le_bytes());
        for record in records {
            match record {
                Record::Duplicate(index) => {
                    buf.push(*index);
                    buf.extend_from_slice(&[0u8; 7]);
                }
                Record::Account {
                    key,
                    owner,
                    signer,
                    writable,
                    executable,
                    lamports,
                    data,
                } => {
                    buf.push(NON_DUP_MARKER);
                    buf.push(*signer as u8);
                    buf.push(*writable as u8);
                    buf.push(*executable as u8);
                    buf.extend_from_slice(&[0u8; 4]);
                    buf.extend_from_slice(&[*key; 32]);
                    buf.extend_from_slice(&[*owner; 32]);
                    buf.extend_from_slice(&lamports.to_le_bytes());
                    buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
                    buf.extend_from_slice(data);
                    buf.extend_from_slice(&vec![0u8; MAX_PERMITTED_DATA_INCREASE]);
                    while !buf.len().is_multiple_of(BPF_ALIGN_OF_U128) {
                        buf.push(0);
                    }
                    buf.extend_from_slice(&0u64.to_le_bytes());
                }
            }
        }
        buf.extend_from_slice(&(instruction_data.len() as u64).to_le_bytes());
        buf.extend_from_slice(instruction_data);
        buf.extend_from_slice(&[program_id; 32]);

        let mut words = vec![0u64; buf.len().div_ceil(8)];
        // SAFETY: `words` holds at least `buf.len()` bytes and cannot overlap `buf`.
        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), words.as_mut_ptr() as *mut u8, buf.len());
        }
        words
    }

    type Summary = (Vec<u8>, Vec<u8>, bool, bool, bool, u64, Vec<u8>);

    fn summarize(info: &AccountInfo<'_>) -> Summary {
        (
            info.key.to_bytes().to_vec(),
            info.owner.to_bytes().to_vec(),
            info.is_signer,
            info.is_writable,
            info.executable,
            **info.lamports.borrow(),
            info.data.borrow().to_vec(),
        )
    }

    #[test]
    fn matches_the_stock_deserializer_including_duplicates() {
        let records = [
            account(1, true, true, 500, b"hello"),
            account(2, false, false, 7, b""),
            Record::Duplicate(0),
            account(4, false, true, u64::MAX, &[0xFF; 33]),
            Record::Duplicate(1),
        ];
        let mut ours_input = serialize(&records, &[1, 2, 3, 4, 5], 0x77);
        let mut stock_input = ours_input.clone();

        let (our_program, our_accounts, our_data, _) =
            unsafe { deserialize(ours_input.as_mut_ptr() as *mut u8) }.unwrap();
        let (stock_program, stock_accounts, stock_data) =
            unsafe { solana_program_entrypoint::deserialize(stock_input.as_mut_ptr() as *mut u8) };

        assert!(our_program.to_bytes() == stock_program.to_bytes());
        assert!(our_data == stock_data);
        assert!(our_accounts.len() == stock_accounts.len());
        for (ours, stock) in our_accounts.iter().zip(stock_accounts.iter()) {
            assert!(summarize(ours) == summarize(stock));
        }
        assert!(Rc::ptr_eq(
            &our_accounts[2].lamports,
            &our_accounts[0].lamports
        ));
        assert!(Rc::ptr_eq(&our_accounts[4].data, &our_accounts[1].data));
        assert!(ours_input == stock_input);
    }

    #[test]
    fn parses_an_empty_account_list() {
        let mut input = serialize(&[], &[9], 0x11);
        let (program, accounts, data, duplicates) =
            unsafe { deserialize(input.as_mut_ptr() as *mut u8) }.unwrap();
        assert!(accounts.is_empty());
        assert!(data == [9]);
        assert!(program.to_bytes() == [0x11u8; 32]);
        assert!(duplicates.is_none());
    }

    fn slot_mask(slots: &[usize]) -> [u64; 4] {
        slots.iter().fold([0u64; 4], |mask, &slot| {
            crate::cursor::mut_mask_set_bit(mask, slot)
        })
    }

    /// The repeats `deserialize` reports for `records`, as the slots involved
    /// in any repeat and the slots that repeat an earlier one.
    fn reported(records: &[Record], program_id: u8) -> (Vec<usize>, Vec<usize>) {
        let mut input = serialize(records, &[], program_id);
        let (_, accounts, _, duplicates) =
            unsafe { deserialize(input.as_mut_ptr() as *mut u8) }.unwrap();
        let Some(duplicates) = duplicates else {
            return (Vec::new(), Vec::new());
        };
        let involved = (0..accounts.len())
            .filter(|&slot| duplicates.check(&slot_mask(&[slot]), &[0; 4]).is_err())
            .collect();
        let repeating = (0..accounts.len())
            .filter(|&slot| duplicates.check(&[0; 4], &slot_mask(&[slot])).is_err())
            .collect();
        (involved, repeating)
    }

    /// The pairwise comparison the solana runtime's dispatch used before the
    /// deserializer reported repeats.
    fn by_pointer_comparison(
        accounts: &[AccountInfo<'_>],
        program_id: &Address,
    ) -> (Vec<usize>, Vec<usize>) {
        let (mut involved, mut repeating) = (Vec::new(), Vec::new());
        for i in 0..accounts.len() {
            for j in 0..i {
                if core::ptr::eq(accounts[i].key, accounts[j].key) && accounts[i].key != program_id
                {
                    for slot in [i, j] {
                        if !involved.contains(&slot) {
                            involved.push(slot);
                        }
                    }
                    if !repeating.contains(&i) {
                        repeating.push(i);
                    }
                }
            }
        }
        involved.sort_unstable();
        repeating.sort_unstable();
        (involved, repeating)
    }

    #[test]
    fn no_repeats_are_reported_for_distinct_accounts() {
        let records = [
            account(1, false, false, 0, b""),
            account(2, false, true, 0, b""),
        ];
        assert!(reported(&records, 0x77) == (Vec::new(), Vec::new()));
    }

    #[test]
    fn a_repeat_involves_both_slots_and_names_the_later_one() {
        let records = [
            account(1, false, false, 0, b""),
            account(2, false, false, 0, b""),
            Record::Duplicate(0),
        ];
        assert!(reported(&records, 0x77) == (vec![0, 2], vec![2]));
    }

    #[test]
    fn an_account_repeated_twice_reports_every_slot_it_occupies() {
        let records = [
            account(1, false, false, 0, b""),
            Record::Duplicate(0),
            account(2, false, false, 0, b""),
            Record::Duplicate(0),
        ];
        assert!(reported(&records, 0x77) == (vec![0, 1, 3], vec![1, 3]));
    }

    #[test]
    fn a_repeat_of_the_program_id_is_not_reported() {
        let records = [
            account(0x77, false, false, 0, b""),
            Record::Duplicate(0),
            account(2, false, false, 0, b""),
        ];
        assert!(reported(&records, 0x77) == (Vec::new(), Vec::new()));
    }

    #[test]
    fn a_program_id_repeat_does_not_hide_another_accounts_repeat() {
        let records = [
            account(0x77, false, false, 0, b""),
            account(2, false, false, 0, b""),
            Record::Duplicate(0),
            Record::Duplicate(1),
        ];
        assert!(reported(&records, 0x77) == (vec![1, 3], vec![3]));
    }

    #[test]
    fn the_report_equals_the_pairwise_pointer_comparison() {
        let layouts: [&[Option<usize>]; 5] = [
            &[None, None, Some(0), None, Some(1), Some(0)],
            &[None, Some(0), Some(0), Some(0)],
            &[None, None, None, Some(2), Some(2)],
            &[None, Some(0), None, Some(2), Some(1)],
            &[None, None, None],
        ];
        for layout in layouts {
            let records: Vec<Record> = layout
                .iter()
                .enumerate()
                .map(|(slot, repeats)| match repeats {
                    Some(first) => Record::Duplicate(*first as u8),
                    None => account(slot as u8 + 1, false, false, 0, b""),
                })
                .collect();
            let mut input = serialize(&records, &[], 0x77);
            let (program_id, accounts, _, _) =
                unsafe { deserialize(input.as_mut_ptr() as *mut u8) }.unwrap();
            assert!(reported(&records, 0x77) == by_pointer_comparison(&accounts, program_id));
        }
    }

    #[test]
    fn accepts_the_maximum_account_count_and_rejects_one_more() {
        let records: Vec<Record> = (0..MAX_ACCOUNTS)
            .map(|_| account(1, false, false, 0, b""))
            .collect();
        let mut input = serialize(&records, &[], 0);
        let (_, accounts, _, _) = unsafe { deserialize(input.as_mut_ptr() as *mut u8) }.unwrap();
        assert!(accounts.len() == MAX_ACCOUNTS);

        let mut over_limit = vec![(MAX_ACCOUNTS as u64) + 1];
        assert!(unsafe { deserialize(over_limit.as_mut_ptr() as *mut u8) }.is_none());
        let mut huge = vec![u64::MAX];
        assert!(unsafe { deserialize(huge.as_mut_ptr() as *mut u8) }.is_none());
    }

    #[test]
    fn rejects_a_duplicate_marker_that_names_no_earlier_account() {
        for records in [
            vec![Record::Duplicate(0)],
            vec![account(1, true, true, 1, b""), Record::Duplicate(1)],
            vec![account(1, true, true, 1, b""), Record::Duplicate(200)],
            vec![account(1, true, true, 1, b""), Record::Duplicate(254)],
        ] {
            let mut input = serialize(&records, &[], 0);
            assert!(unsafe { deserialize(input.as_mut_ptr() as *mut u8) }.is_none());
        }
    }
}
