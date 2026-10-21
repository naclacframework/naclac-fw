#!/usr/bin/env bash
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_file="$repo_root/test-output-$(date +%Y%m%d-%H%M%S).log"

build_first_suites="cpi"

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

    if [ -f "${case_dir}variants/variants.conf" ]; then
        echo "=== $case_name :: generate variants ==="
        if ! bash "$repo_root/scripts/gen-variants.sh" "$case_dir"; then
            echo "FAILED: $case_name :: generate variants"
            fail=1
            echo
            continue
        fi
        echo
    fi

    if [ -f "${case_dir}Naclac.toml" ]; then
        steps=(check build check-test)
        case " $build_first_suites " in
            *" $case_name "*) steps=(build check check-test) ;;
        esac

        step_failed=0
        for step in "${steps[@]}"; do
            case $step in
                check) label="naclac check"; args=(check --deny-warnings) ;;
                build) label="naclac build"; args=(build --show-output) ;;
                check-test) label="naclac check --test"; args=(check --test --deny-warnings) ;;
            esac
            echo "=== $case_name :: $label ==="
            if ! (cd "$case_dir" && naclac "${args[@]}"); then
                echo "FAILED: $case_name :: $label"
                fail=1
                step_failed=1
                echo
                break
            fi
            echo
        done
        [ "$step_failed" -eq 0 ] || continue
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
