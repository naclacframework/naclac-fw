#!/usr/bin/env bash
# Builds layout_bench for SBF and prints how many instructions each exported
# function compiles to, native vs alignment-1. The functions have no loops,
# and one instruction costs one compute unit, so the count of a branch-free
# body is its exact cost, and for the checked operations it is the cost of the
# longest path.
#
# Uses a platform-tools version that is already cached (the same versions and
# arch `naclac build` accepts) and never downloads one. Set TOOLS_VERSION to
# pick a cached version, e.g. TOOLS_VERSION=v1.56, and ARCH to pick the SBF
# arch (v0 to v4, default v3).
#
# Usage: tests/layout-bench/run.sh

set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CACHE="$HOME/.cache/solana"

TOOLS_VERSION=${TOOLS_VERSION:-}
if [ -z "$TOOLS_VERSION" ]; then
  for candidate in v1.58 v1.56; do
    if [ -d "$CACHE/$candidate/platform-tools" ]; then
      TOOLS_VERSION=$candidate
      break
    fi
  done
fi
if [ -z "$TOOLS_VERSION" ] || [ ! -d "$CACHE/$TOOLS_VERSION/platform-tools" ]; then
  echo "no cached platform-tools found under $CACHE (looked for v1.58, v1.56)" >&2
  echo "available: $(ls "$CACHE" 2>/dev/null | tr '\n' ' ')" >&2
  exit 1
fi
echo "using platform-tools $TOOLS_VERSION" >&2

OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT

ARCH=${ARCH:-v3}
echo "using arch $ARCH" >&2

CARGO_TARGET_DIR="$OUT/target" cargo build-sbf --tools-version "$TOOLS_VERSION" --arch "$ARCH" \
  --manifest-path "$HERE/Cargo.toml" --sbf-out-dir "$OUT/deploy" >/dev/null

# The deployable .so in $OUT/deploy is stripped; the copy cargo leaves in its
# release directory still has the symbol table that names each function.
SO=$(find "$OUT/target" -path '*/release/layout_bench.so' | head -n 1)
OBJDUMP=$(find "$CACHE/$TOOLS_VERSION" -name 'llvm-objdump' 2>/dev/null | head -n 1)
if [ -z "$SO" ] || [ -z "$OBJDUMP" ]; then
  echo "could not find the unstripped layout_bench.so or llvm-objdump in platform-tools $TOOLS_VERSION" >&2
  exit 1
fi

if [ -n "${RAW:-}" ]; then
  "$OBJDUMP" -d "$SO" | head -n "$RAW"
  exit 0
fi

# Functions are located by the symbol table (`-t`), which lists each
# exported function's address and size. A function the compiler folded into an
# identical one shares that one's address. For each operation in OPS the table
# shows the native, wrapper and packed-struct instruction counts; "identical"
# means all three bodies are the same instructions.
OPS="read_u64 read_u32 read_u16 write_u64 add_u64 checked_add_u64 transfer sum_all eq_u64 everyday"

if [ -n "${SYMS:-}" ]; then
  "$OBJDUMP" -t "$SO"
  exit 0
fi

{
  "$OBJDUMP" -t "$SO" | sed 's/^/SYM /'
  "$OBJDUMP" -d "$SO" | sed 's/^/INS /'
} | awk -v ops="$OPS" '
  function hex(s,    i, c, v) {
    v = 0
    s = tolower(s)
    sub(/^0x/, "", s)
    for (i = 1; i <= length(s); i++) {
      c = index("0123456789abcdef", substr(s, i, 1))
      if (c == 0) return -1
      v = v * 16 + c - 1
    }
    return v
  }
  $1 == "SYM" && $NF ~ /^(native|packed|repacked)_/ {
    start[$NF] = hex($2)
    size[$NF] = hex($(NF - 1))
    next
  }
  $1 == "INS" && $2 ~ /^[0-9a-f]+:$/ {
    addr = $2
    sub(/:$/, "", addr)
    line = $0
    sub(/^INS[[:space:]]+[0-9a-f]+:[[:space:]]+([0-9a-f][0-9a-f] )+[[:space:]]*/, "", line)
    sub(/ <.*>$/, "", line)
    sub(/[+-]0x[0-9a-f]+$/, "", line)
    n_ins++
    ins_addr[n_ins] = hex(addr)
    ins_text[n_ins] = line
  }
  END {
    for (f in start) {
      for (i = 1; i <= n_ins; i++) {
        if (ins_addr[i] >= start[f] && ins_addr[i] < start[f] + size[f]) {
          count[f]++
          body[f] = body[f] "|" ins_text[i]
        }
      }
    }
    n = split(ops, names, " ")
    printf "%-18s %8s %8s %8s  %s\n", "operation", "native", "wrapper", "packed", "code"
    missing = 0
    for (i = 1; i <= n; i++) {
      a = "native_" names[i]; b = "packed_" names[i]; c = "repacked_" names[i]
      if (!(a in start) || !(b in start) || !(c in start)) {
        printf "%-18s  missing from the symbol table\n", names[i]
        missing = 1
        continue
      }
      same = (body[a] == body[b] && body[a] == body[c]) ? "identical" : "differs"
      printf "%-18s %8d %8d %8d  %s\n", names[i], count[a], count[b], count[c], same
    }
    if (missing) {
      print "some functions are missing; run with SYMS=1 to see the symbol table"
      exit 1
    }
  }
'
