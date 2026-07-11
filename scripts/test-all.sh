#!/usr/bin/env bash
set -uo pipefail

# Runs every framework regression test under tests/*/, one Cargo package at a
# time.
#
# Why not `cargo test --workspace` per case: sibling packages in the same
# test-case workspace intentionally request conflicting naclac-lang features
# (pinocchio vs borsh vs plain solana-zerocopy — see tests/discriminator for
# the concrete example). `--workspace` unifies features across the whole
# resolve, so building all three at once silently corrupts every mode
# simultaneously instead of testing them. Testing one package at a time gives
# each mode its own clean feature resolution, matching what `naclac test`
# already does internally.
#
# Also sets CARGO_TARGET_DIR to a native Linux path (under WSL) per case,
# scoped to just the `cargo test` calls this script makes. Deliberately not
# a `.cargo/config.toml` in each case dir — that would apply to `naclac
# build`'s internal `cargo build-sbf` too, silently moving `target/deploy/
# *.so` off the relative path every test file's `load_program()` helper
# expects. Redirecting only here keeps SBF output at the normal location
# while still avoiding NTFS-via-9p `Permission denied` failures on
# /mnt/c/... paths during `cargo test`'s own host-arch build.
#
# Usage: scripts/test-all.sh [case-name ...]
#   No args  -> run every case under tests/
#   With args -> run only the named case(s), e.g. scripts/test-all.sh discriminator

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
native_target_root="${NACLAC_TEST_TARGET_ROOT:-$HOME/naclac-test-targets}"
fail=0
ran_any=0

if [ "$#" -gt 0 ]; then
    case_dirs=()
    for name in "$@"; do
        case_dirs+=("$repo_root/tests/$name/")
    done
else
    case_dirs=("$repo_root"/tests/*/)
fi

for case_dir in "${case_dirs[@]}"; do
    [ -f "${case_dir}Cargo.toml" ] || continue
    case_name="$(basename "$case_dir")"

    for pkg_toml in "$case_dir"programs/*/Cargo.toml; do
        [ -f "$pkg_toml" ] || continue
        pkg_name="$(grep -m1 '^name' "$pkg_toml" | sed -E 's/name *= *"(.*)"/\1/')"
        ran_any=1

        echo "=== $case_name :: $pkg_name ==="
        if ! (cd "$case_dir" && CARGO_TARGET_DIR="$native_target_root/$case_name" cargo test -p "$pkg_name" -- --nocapture); then
            echo "FAILED: $case_name :: $pkg_name"
            fail=1
        fi
        echo
    done
done

if [ "$ran_any" -eq 0 ]; then
    echo "No test cases found under tests/ (or none matched: $*)" >&2
    exit 1
fi

exit $fail
