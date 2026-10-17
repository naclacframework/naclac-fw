#!/usr/bin/env bash
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_file="$repo_root/test-output-$(date +%Y%m%d-%H%M%S).log"

run_all() {
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

    if [ -f "${case_dir}Naclac.toml" ]; then
        echo "=== $case_name :: naclac check ==="
        if ! (cd "$case_dir" && naclac check --deny-warnings); then
            echo "FAILED: $case_name :: naclac check"
            fail=1
            echo
            continue
        fi
        echo

        echo "=== $case_name :: naclac build ==="
        if ! (cd "$case_dir" && naclac build --show-output); then
            echo "FAILED: $case_name :: naclac build"
            fail=1
            echo
            continue
        fi
        echo

        echo "=== $case_name :: naclac check --test ==="
        if ! (cd "$case_dir" && naclac check --test --deny-warnings); then
            echo "FAILED: $case_name :: naclac check --test"
            fail=1
            echo
            continue
        fi
        echo
    fi

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
    return 1
fi

return $fail
}

run_all "$@" 2>&1 | tee "$log_file"
exit_code=${PIPESTATUS[0]}
echo
echo "Full log written to: $log_file"
exit "$exit_code"
