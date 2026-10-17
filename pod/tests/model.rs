//! Model-based tests: long random sequences of operations run against
//! `FixedVec`/`FixedString` and against std's `Vec`/`String`, and after every
//! step the two must agree and the fixed type's bytes must match its canonical
//! form (the length, the contents, and zero everywhere after them).
//!
//! The random numbers come from a fixed-seed generator, so every run is the
//! same and a failure names the seed and step that reproduce it.

use {
    bytemuck::{Pod, Zeroable},
    naclac_pod::{FixedString, FixedVec, PodError},
};

/// How many seeds and steps to run. Miri checks every memory access and runs
/// about a thousand times slower, so it gets a small share of the full volume;
/// the same code paths run either way.
fn seeds(full: u64) -> u64 {
    if cfg!(miri) {
        2
    } else {
        full
    }
}

fn steps(full: usize) -> usize {
    if cfg!(miri) {
        (full / 60).max(25)
    } else {
        full
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn canonical<T: Pod, const N: usize>(model: &[T]) -> Vec<u8> {
    let mut bytes = vec![0u8; 2 + N * core::mem::size_of::<T>()];
    bytes[..2].copy_from_slice(&(model.len() as u16).to_le_bytes());
    let elements: &[u8] = bytemuck::cast_slice(model);
    bytes[2..2 + elements.len()].copy_from_slice(elements);
    bytes
}

fn check_vec<T: Pod + PartialEq + core::fmt::Debug, const N: usize>(
    list: &FixedVec<T, N>,
    model: &[T],
    context: &str,
) {
    assert_eq!(list.len(), model.len(), "{context}: length");
    assert_eq!(list.is_empty(), model.is_empty(), "{context}: is_empty");
    assert_eq!(list.is_full(), model.len() == N, "{context}: is_full");
    let collected: Vec<T> = list.iter().collect();
    assert_eq!(collected, model, "{context}: contents");
    assert_eq!(
        bytemuck::bytes_of(list),
        canonical::<T, N>(model).as_slice(),
        "{context}: canonical bytes"
    );
    for (index, expected) in model.iter().enumerate() {
        assert_eq!(list.get(index), Some(*expected), "{context}: get({index})");
    }
    assert_eq!(list.get(model.len()), None, "{context}: get(len)");
    assert_eq!(list.first(), model.first().copied(), "{context}: first");
    assert_eq!(list.last(), model.last().copied(), "{context}: last");
}

fn run_vec_model<T, const N: usize>(seed: u64, steps: usize, make: impl Fn(&mut Rng) -> T)
where
    T: Pod + PartialEq + core::fmt::Debug,
{
    let mut rng = Rng(seed);
    let mut list = FixedVec::<T, N>::new();
    let mut model: Vec<T> = Vec::new();

    for step in 0..steps {
        let context = format!("seed {seed} step {step}");
        match rng.below(12) {
            0 => {
                let value = make(&mut rng);
                let result = list.try_push(value);
                if model.len() < N {
                    assert_eq!(result, Ok(()), "{context}: push");
                    model.push(value);
                } else {
                    assert_eq!(result, Err(PodError::CapacityExceeded), "{context}: push");
                }
            }
            1 => {
                assert_eq!(list.pop(), model.pop(), "{context}: pop");
            }
            2 => {
                let index = rng.below(N + 2);
                let value = make(&mut rng);
                let result = list.try_insert(index, value);
                if model.len() < N && index <= model.len() {
                    assert_eq!(result, Ok(()), "{context}: insert");
                    model.insert(index, value);
                } else {
                    assert_eq!(result, Err(PodError::CapacityExceeded), "{context}: insert");
                }
            }
            3 => {
                let index = rng.below(N + 2);
                let expected = (index < model.len()).then(|| model.remove(index));
                assert_eq!(list.remove(index), expected, "{context}: remove");
            }
            4 => {
                let index = rng.below(N + 2);
                let expected = (index < model.len()).then(|| model.swap_remove(index));
                assert_eq!(list.swap_remove(index), expected, "{context}: swap_remove");
            }
            5 => {
                let new_len = rng.below(N + 2);
                list.truncate(new_len);
                model.truncate(new_len);
            }
            6 => {
                list.clear();
                model.clear();
            }
            7 => {
                let index = rng.below(N + 2);
                let value = make(&mut rng);
                let ok = list.set(index, value);
                assert_eq!(ok, index < model.len(), "{context}: set");
                if ok {
                    model[index] = value;
                }
            }
            8 => {
                let count = rng.below(N + 2);
                let values: Vec<T> = (0..count).map(|_| make(&mut rng)).collect();
                let result = list.try_set_from_slice(&values);
                if count <= N {
                    assert_eq!(result, Ok(()), "{context}: set_from_slice");
                    model = values;
                } else {
                    assert_eq!(
                        result,
                        Err(PodError::CapacityExceeded),
                        "{context}: set_from_slice"
                    );
                }
            }
            9 => {
                let count = rng.below(N + 2);
                let values: Vec<T> = (0..count).map(|_| make(&mut rng)).collect();
                let result = list.try_extend_from_slice(&values);
                if model.len() + count <= N {
                    assert_eq!(result, Ok(()), "{context}: extend");
                    model.extend(values);
                } else {
                    assert_eq!(result, Err(PodError::CapacityExceeded), "{context}: extend");
                }
            }
            10 => {
                let threshold = rng.below(3);
                let keep = |value: T| (bytemuck::bytes_of(&value)[0] as usize % 3) != threshold;
                list.retain(keep);
                model.retain(|value| keep(*value));
            }
            _ => {
                let reversed: Vec<T> = list.iter().rev().collect();
                let mut expected = model.clone();
                expected.reverse();
                assert_eq!(reversed, expected, "{context}: reverse iteration");
                let mut out = vec![T::zeroed(); model.len()];
                assert!(list.copy_to_slice(&mut out), "{context}: copy_to_slice");
                assert_eq!(out, model, "{context}: copy_to_slice contents");
                if let Some(first) = model.first() {
                    assert!(list.contains(first), "{context}: contains");
                }
            }
        }
        check_vec(&list, &model, &context);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct Mixed {
    wide: u64,
    narrow: u16,
    flag: u8,
}

#[test]
fn fixed_vec_of_bytes_matches_vec() {
    for seed in 1..=seeds(48) {
        run_vec_model::<u8, 7>(seed, steps(1500), |rng| rng.next() as u8);
    }
}

#[test]
fn fixed_vec_of_u16_matches_vec() {
    for seed in 100..=99 + seeds(49) {
        run_vec_model::<u16, 9>(seed, steps(1500), |rng| rng.next() as u16);
    }
}

#[test]
fn fixed_vec_of_u64_matches_vec() {
    for seed in 200..=199 + seeds(49) {
        run_vec_model::<u64, 6>(seed, steps(1500), |rng| rng.next());
    }
}

#[test]
fn fixed_vec_of_u128_matches_vec() {
    for seed in 300..=299 + seeds(33) {
        run_vec_model::<u128, 5>(seed, steps(1200), |rng| {
            (u128::from(rng.next()) << 64) | u128::from(rng.next())
        });
    }
}

#[test]
fn fixed_vec_of_addresses_sized_arrays_matches_vec() {
    for seed in 400..=399 + seeds(33) {
        run_vec_model::<[u8; 32], 4>(seed, steps(1200), |rng| {
            let mut value = [0u8; 32];
            for byte in &mut value {
                *byte = rng.next() as u8;
            }
            value
        });
    }
}

#[test]
fn fixed_vec_of_packed_structs_matches_vec() {
    for seed in 500..=499 + seeds(49) {
        run_vec_model::<Mixed, 5>(seed, steps(1500), |rng| Mixed {
            wide: rng.next(),
            narrow: rng.next() as u16,
            flag: rng.next() as u8,
        });
    }
}

#[test]
fn fixed_vec_with_capacity_one_and_zero() {
    for seed in 600..=599 + seeds(17) {
        run_vec_model::<u32, 1>(seed, steps(800), |rng| rng.next() as u32);
        run_vec_model::<u32, 0>(seed, steps(200), |rng| rng.next() as u32);
    }
}

const ALPHABET: [&str; 6] = ["a", "Z", "é", "日", "😀", " "];

fn random_text(rng: &mut Rng, max_chars: usize) -> String {
    (0..rng.below(max_chars + 1))
        .map(|_| ALPHABET[rng.below(ALPHABET.len())])
        .collect()
}

fn canonical_string<const N: usize>(model: &str) -> Vec<u8> {
    let mut bytes = vec![0u8; 2 + N];
    bytes[..2].copy_from_slice(&(model.len() as u16).to_le_bytes());
    bytes[2..2 + model.len()].copy_from_slice(model.as_bytes());
    bytes
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut cut = index.min(text.len());
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    cut
}

fn run_string_model<const N: usize>(seed: u64, steps: usize) {
    let mut rng = Rng(seed);
    let mut text = FixedString::<N>::new();
    let mut model = String::new();

    for step in 0..steps {
        let context = format!("seed {seed} step {step}");
        match rng.below(6) {
            0 => {
                let value = random_text(&mut rng, 6);
                let result = text.try_set(&value);
                if value.len() <= N {
                    assert_eq!(result, Ok(()), "{context}: set");
                    model = value;
                } else {
                    assert_eq!(result, Err(PodError::CapacityExceeded), "{context}: set");
                }
            }
            1 => {
                let value = random_text(&mut rng, 4);
                let result = text.try_push_str(&value);
                if model.len() + value.len() <= N {
                    assert_eq!(result, Ok(()), "{context}: push_str");
                    model.push_str(&value);
                } else {
                    assert_eq!(
                        result,
                        Err(PodError::CapacityExceeded),
                        "{context}: push_str"
                    );
                }
            }
            2 => {
                let new_len = rng.below(N + 3);
                text.truncate(new_len);
                if new_len < model.len() {
                    let cut = floor_char_boundary(&model, new_len);
                    model.truncate(cut);
                }
            }
            3 => {
                text.clear();
                model.clear();
            }
            4 => {
                let other = random_text(&mut rng, 3);
                assert_eq!(text.eq_str(&other), model == other, "{context}: eq_str");
            }
            _ => {
                assert_eq!(text.len(), model.len(), "{context}: len");
            }
        }
        assert_eq!(text.as_str(), Ok(model.as_str()), "{context}: text");
        assert_eq!(text.len(), model.len(), "{context}: len");
        assert_eq!(
            bytemuck::bytes_of(&text),
            canonical_string::<N>(&model).as_slice(),
            "{context}: canonical bytes"
        );
    }
}

#[test]
fn fixed_string_matches_string() {
    for seed in 700..=699 + seeds(61) {
        run_string_model::<16>(seed, steps(1500));
        run_string_model::<5>(seed, steps(800));
        run_string_model::<1>(seed, steps(300));
        run_string_model::<0>(seed, steps(100));
    }
}

#[test]
fn reading_arbitrary_bytes_as_a_list_or_string_never_misbehaves() {
    let mut rng = Rng(0xDEAD_BEEF);
    for round in 0..steps(2000) {
        let mut bytes = [0u8; 2 + 8];
        for byte in &mut bytes {
            *byte = rng.next() as u8;
        }
        let list: FixedVec<u8, 8> = bytemuck::pod_read_unaligned(&bytes);
        assert!(list.len() <= 8, "round {round}");
        assert_eq!(list.iter().count(), list.len(), "round {round}");
        assert_eq!(list.as_slice().len(), list.len(), "round {round}");

        let text: FixedString<8> = bytemuck::pod_read_unaligned(&bytes);
        assert!(text.len() <= 8, "round {round}");
        assert_eq!(
            text.as_str().is_ok(),
            std::str::from_utf8(text.as_bytes()).is_ok()
        );

        let mut mutated = list;
        while mutated.pop().is_some() {}
        assert!(mutated.is_empty());
        let _ = mutated.try_push(1);
    }
}
