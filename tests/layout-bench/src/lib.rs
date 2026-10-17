//! Compares what the same account-field operations cost on SBF in three
//! layouts: native fields (`Native`, naclac's current layout), alignment-1
//! wrapper fields (`Packed`, Quasar's layout: every integer is a byte array
//! read and written with `from_le_bytes`/`to_le_bytes`, as in zeropod's
//! `PodU64`), and plain integer fields in a `#[repr(C, packed)]` struct
//! (`Repacked`, alignment 1 with no wrapper types).
//!
//! Each function is exported unmangled and never inlined so `run.sh` can count
//! the instructions of its compiled body. For every operation the functions
//! are defined in the order `native_<op>`, `packed_<op>`, `repacked_<op>`.
//! That `Repacked` compiles at all is part of the result: its functions use
//! plain field syntax (`+=`, `==`, `>`, `.min()`, `.checked_add()`).

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[repr(C)]
pub struct Native {
    pub a: u64,
    pub b: u64,
    pub c: u32,
    pub d: u16,
    pub e: u8,
    pub f: u8,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PodU64([u8; 8]);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PodU32([u8; 4]);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PodU16([u8; 2]);

impl PodU64 {
    #[inline(always)]
    fn get(&self) -> u64 {
        u64::from_le_bytes(self.0)
    }
    #[inline(always)]
    fn set(&mut self, value: u64) {
        self.0 = value.to_le_bytes();
    }
}

impl PodU32 {
    #[inline(always)]
    fn get(&self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

impl PodU16 {
    #[inline(always)]
    fn get(&self) -> u16 {
        u16::from_le_bytes(self.0)
    }
}

#[repr(C)]
pub struct Packed {
    pub a: PodU64,
    pub b: PodU64,
    pub c: PodU32,
    pub d: PodU16,
    pub e: u8,
    pub f: u8,
}

#[repr(C, packed)]
pub struct Repacked {
    pub a: u64,
    pub b: u64,
    pub c: u32,
    pub d: u16,
    pub e: u8,
    pub f: u8,
}

const _: () = assert!(core::mem::size_of::<Native>() == core::mem::size_of::<Packed>());
const _: () = assert!(core::mem::size_of::<Native>() == core::mem::size_of::<Repacked>());
const _: () = assert!(core::mem::align_of::<Packed>() == 1);
const _: () = assert!(core::mem::align_of::<Repacked>() == 1);

// --- read_u64 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_read_u64(p: &Native) -> u64 {
    p.a
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_read_u64(p: &Packed) -> u64 {
    p.a.get()
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_read_u64(p: &Repacked) -> u64 {
    p.a
}

// --- read_u32 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_read_u32(p: &Native) -> u32 {
    p.c
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_read_u32(p: &Packed) -> u32 {
    p.c.get()
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_read_u32(p: &Repacked) -> u32 {
    p.c
}

// --- read_u16 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_read_u16(p: &Native) -> u16 {
    p.d
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_read_u16(p: &Packed) -> u16 {
    p.d.get()
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_read_u16(p: &Repacked) -> u16 {
    p.d
}

// --- write_u64 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_write_u64(p: &mut Native, value: u64) {
    p.a = value;
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_write_u64(p: &mut Packed, value: u64) {
    p.a.set(value);
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_write_u64(p: &mut Repacked, value: u64) {
    p.a = value;
}

// --- add_u64 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_add_u64(p: &mut Native, value: u64) {
    p.a = p.a.wrapping_add(value);
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_add_u64(p: &mut Packed, value: u64) {
    p.a.set(p.a.get().wrapping_add(value));
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_add_u64(p: &mut Repacked, value: u64) {
    p.a = p.a.wrapping_add(value);
}

// --- checked_add_u64 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_checked_add_u64(p: &mut Native, value: u64) -> u64 {
    match p.a.checked_add(value) {
        Some(sum) => {
            p.a = sum;
            1
        }
        None => 0,
    }
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_checked_add_u64(p: &mut Packed, value: u64) -> u64 {
    match p.a.get().checked_add(value) {
        Some(sum) => {
            p.a.set(sum);
            1
        }
        None => 0,
    }
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_checked_add_u64(p: &mut Repacked, value: u64) -> u64 {
    match p.a.checked_add(value) {
        Some(sum) => {
            p.a = sum;
            1
        }
        None => 0,
    }
}

// --- transfer ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_transfer(p: &mut Native, value: u64) -> u64 {
    match p.a.checked_sub(value) {
        Some(from) => {
            p.a = from;
            p.b = p.b.wrapping_add(value);
            1
        }
        None => 0,
    }
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_transfer(p: &mut Packed, value: u64) -> u64 {
    match p.a.get().checked_sub(value) {
        Some(from) => {
            p.a.set(from);
            p.b.set(p.b.get().wrapping_add(value));
            1
        }
        None => 0,
    }
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_transfer(p: &mut Repacked, value: u64) -> u64 {
    match p.a.checked_sub(value) {
        Some(from) => {
            p.a = from;
            p.b = p.b.wrapping_add(value);
            1
        }
        None => 0,
    }
}

// --- sum_all ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_sum_all(p: &Native) -> u64 {
    p.a.wrapping_add(p.b)
        .wrapping_add(u64::from(p.c))
        .wrapping_add(u64::from(p.d))
        .wrapping_add(u64::from(p.e))
        .wrapping_add(u64::from(p.f))
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_sum_all(p: &Packed) -> u64 {
    p.a.get()
        .wrapping_add(p.b.get())
        .wrapping_add(u64::from(p.c.get()))
        .wrapping_add(u64::from(p.d.get()))
        .wrapping_add(u64::from(p.e))
        .wrapping_add(u64::from(p.f))
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_sum_all(p: &Repacked) -> u64 {
    p.a.wrapping_add(p.b)
        .wrapping_add(u64::from(p.c))
        .wrapping_add(u64::from(p.d))
        .wrapping_add(u64::from(p.e))
        .wrapping_add(u64::from(p.f))
}

// --- eq_u64 ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_eq_u64(p: &Native, q: &Native) -> u64 {
    u64::from(p.a == q.a)
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_eq_u64(p: &Packed, q: &Packed) -> u64 {
    u64::from(p.a.0 == q.a.0)
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_eq_u64(p: &Repacked, q: &Repacked) -> u64 {
    u64::from(p.a == q.a)
}

// --- compile check only (not counted by run.sh): a packed struct nested in a
// packed struct, an array of them, and a reference to an alignment-1 field ---

#[repr(C, packed)]
pub struct Inner {
    pub x: u64,
    pub y: u32,
}

#[repr(C, packed)]
pub struct Outer {
    pub tag: u8,
    pub inner: Inner,
    pub list: [Inner; 2],
    pub key: [u8; 32],
}

const _: () = assert!(core::mem::align_of::<Outer>() == 1);
const _: () = assert!(core::mem::size_of::<Outer>() == 1 + 12 + 24 + 32);

#[no_mangle]
#[inline(never)]
pub extern "C" fn check_nested(o: &mut Outer, v: u64) -> u64 {
    o.inner.x += v;
    o.list[1].y = 3;
    let inner: &Inner = &o.inner;
    let key: &[u8; 32] = &o.key;
    let x = inner.x;
    x + u64::from(o.list[0].y) + u64::from(key[0]) + u64::from(o.tag)
}

// --- everyday: `+=`, `>`, `.min()` (overflow checks are off in this crate,
// so no function has a panic path that `run.sh` would count) ---

#[no_mangle]
#[inline(never)]
pub extern "C" fn native_everyday(p: &mut Native, value: u64) -> u64 {
    p.a += value;
    if p.a > 100 {
        p.b = p.a.min(value);
    }
    u64::from(p.c) + u64::from(p.d)
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn packed_everyday(p: &mut Packed, value: u64) -> u64 {
    let a = p.a.get() + value;
    p.a.set(a);
    if a > 100 {
        p.b.set(a.min(value));
    }
    u64::from(p.c.get()) + u64::from(p.d.get())
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn repacked_everyday(p: &mut Repacked, value: u64) -> u64 {
    p.a += value;
    if p.a > 100 {
        p.b = p.a.min(value);
    }
    u64::from(p.c) + u64::from(p.d)
}
