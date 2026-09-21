#!/usr/bin/env bash
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
        if ! (cd "$case_dir" && cargo test -p "$pkg_name" -- --nocapture); then
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
