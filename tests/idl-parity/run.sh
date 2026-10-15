#!/usr/bin/env bash
# IDL parity harness: builds every source tree under sources/ in each representation it supports,
# produces the program's IDL twice (AST walker, then compiled `idl-build`) and diffs the results.
# Nothing under sources/ is modified; everything generated lives in .work/ (gitignored).
#
#   ./run.sh                            all trees, all representations
#   ./run.sh --tree zc                  one tree          (zc | serde | zc_custom | serde_custom | modules,
#                                                          or a probe tree such as probe_accounts)
#   ./run.sh --rep pinocchio            one representation (zc | pinocchio | borsh | wincode)
#   ./run.sh --check-only               only `cargo check` the idl-build copy (compile errors, fast)
#   ./run.sh --ast-only                 only the AST side (seconds, no compilation of the program)
#   ./run.sh --verbose                  stream all cargo / naclac output live (it is always saved in .work/logs/)
#
# Needs: naclac, solana-keygen, python3, cargo on PATH. Exit status is non-zero if any variant failed
# or showed a NEW difference.

set -u -o pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
FW=$(cd "$HERE/../.." && pwd)
WORK="$HERE/.work"
NACLAC=${NACLAC:-naclac}

declare -A REPS_OF=(
  [zc]="zc pinocchio"
  [zc_custom]="zc pinocchio"
  [serde]="borsh wincode"
  [serde_custom]="borsh wincode"
  [modules]="zc pinocchio borsh wincode"
  [probe_accounts]="zc"
)
ALL_TREES=(zc serde zc_custom serde_custom modules)

want_trees=()
want_reps=()
check_only=0
ast_only=0
verbose=0
while [ $# -gt 0 ]; do
  case $1 in
    --tree) want_trees+=("$2"); shift 2 ;;
    --rep) want_reps+=("$2"); shift 2 ;;
    --check-only) check_only=1; shift ;;
    --ast-only) ast_only=1; shift ;;
    --verbose) verbose=1; shift ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

contains() { local needle=$1; shift; for x in "$@"; do [ "$x" = "$needle" ] && return 0; done; return 1; }

target_dir_of() {
  python3 - "$1" <<'PY'
import json, subprocess, sys
out = subprocess.check_output(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", sys.argv[1]]
)
print(json.loads(out)["target_directory"])
PY
}

# materialize TREE REP MODE  -> sets WS, CRATE, TARGET, PUBKEY
materialize() {
  local tree=$1 rep=$2 mode=$3
  CRATE="idlp_${tree}_${rep}"
  WS="$WORK/$mode/$CRATE"
  local prog="$WS/programs/$CRATE"
  rm -rf "$WS"
  mkdir -p "$prog"

  cat > "$WS/Cargo.toml" <<EOF
[workspace]
members = ["programs/*"]
resolver = "2"

[workspace.lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = [
    'cfg(target_os, values("solana"))',
    'cfg(feature, values("pinocchio", "solana", "borsh", "wincode"))'
] }
EOF

  local default_line="" dep_flags='features = ["token", "metadata"]' idl_line=""
  case $rep in
    pinocchio) default_line='default = ["pinocchio"]'; dep_flags='default-features = false, features = ["token", "metadata"]' ;;
    borsh) default_line='default = ["borsh"]' ;;
    wincode) default_line='default = ["wincode"]' ;;
    zc) ;;
  esac
  [ "$mode" = ib ] && idl_line='idl-build = ["naclac-lang/idl-build"]'
  local fw_rel
  fw_rel=$(python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "$FW" "$prog")

  cat > "$prog/Cargo.toml" <<EOF
[package]
name = "$CRATE"
version = "0.1.0"
description = "IDL parity fixture ($tree, $rep, $mode)"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[features]
no-entrypoint = []
cpi = ["no-entrypoint"]
custom-heap = []
custom-panic = []
debug-mode = []
$default_line
pinocchio = ["naclac-lang/pinocchio"]
borsh = ["naclac-lang/borsh"]
wincode = ["naclac-lang/wincode"]
$idl_line

[dependencies]
naclac-lang = { version = "0.1.0", path = "$fw_rel/naclac-lang", $dep_flags }

[lints]
workspace = true
EOF

  cp -r "$HERE/sources/$tree/src" "$prog/src"

  TARGET=$(target_dir_of "$WS/Cargo.toml") || return 1
  mkdir -p "$TARGET/deploy"
  mkdir -p "$WORK/keys"
  local keypair="$WORK/keys/${CRATE}-keypair.json"
  [ -f "$keypair" ] || solana-keygen new --no-bip39-passphrase --silent --force -o "$keypair" >/dev/null || return 1
  cp "$keypair" "$TARGET/deploy/${CRATE}-keypair.json"
  PUBKEY=$(solana-keygen pubkey "$keypair") || return 1

  find "$prog/src" -name '*.rs' -exec sed -i "s/__CRATE__/$CRATE/g; s/__PROGRAM_ID__/$PUBKEY/g" {} +

  cat > "$WS/Naclac.toml" <<EOF
[toolchain]
package_manager = "yarn"

[features]
resolution = true
skip-lint = false

[programs.localnet]
$CRATE = "$PUBKEY"

[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"
EOF
}

# generate_idl OUT LOG  (uses WS CRATE TARGET from materialize)
generate_idl() {
  local out=$1 log=$2
  rm -f "$TARGET/idl/$CRATE.json"
  local rc
  if [ $verbose -eq 1 ]; then
    ( cd "$WS" && "$NACLAC" generate "$CRATE" --show-output idl ) 2>&1 | tee "$log"
    rc=${PIPESTATUS[0]}
  else
    ( cd "$WS" && "$NACLAC" generate "$CRATE" --show-output idl ) > "$log" 2>&1
    rc=$?
  fi
  [ $rc -eq 0 ] || return 1
  [ -f "$TARGET/idl/$CRATE.json" ] || return 1
  cp "$TARGET/idl/$CRATE.json" "$out"
}

mkdir -p "$WORK/out" "$WORK/logs"
failures=0
summary=()

if [ ${#want_trees[@]} -gt 0 ]; then
  run_trees=("${want_trees[@]}")
else
  run_trees=("${ALL_TREES[@]}")
fi
for tree in "${run_trees[@]}"; do
  if [ -z "${REPS_OF[$tree]+set}" ]; then
    echo "unknown tree '$tree' (known: ${!REPS_OF[*]})" >&2
    exit 2
  fi
done

for tree in "${run_trees[@]}"; do
  [ -d "$HERE/sources/$tree/src" ] || { summary+=("$tree: no sources yet, skipped"); continue; }
  for rep in ${REPS_OF[$tree]}; do
    [ ${#want_reps[@]} -gt 0 ] && ! contains "$rep" "${want_reps[@]}" && continue
    id="${tree}_${rep}"
    echo "=== $id"

    if [ $check_only -eq 1 ]; then
      materialize "$tree" "$rep" ib || { echo "  setup failed"; summary+=("$id: SETUP FAILED"); failures=$((failures+1)); continue; }
      if [ $verbose -eq 1 ]; then
        cargo check --manifest-path "$WS/Cargo.toml" -p "$CRATE" --features idl-build 2>&1 | tee "$WORK/logs/$id.check.log"
        rc=${PIPESTATUS[0]}
      else
        cargo check --manifest-path "$WS/Cargo.toml" -p "$CRATE" --features idl-build >"$WORK/logs/$id.check.log" 2>&1
        rc=$?
      fi
      if [ $rc -eq 0 ]; then
        echo "  cargo check: ok"; summary+=("$id: check ok")
      else
        echo "  cargo check FAILED. Compiler errors (full log: .work/logs/$id.check.log):"
        grep -E "^error" -A14 "$WORK/logs/$id.check.log" | head -200
        summary+=("$id: CHECK FAILED (see .work/logs/$id.check.log)"); failures=$((failures+1))
      fi
      continue
    fi

    materialize "$tree" "$rep" ast || { echo "  setup failed"; summary+=("$id: SETUP FAILED"); failures=$((failures+1)); continue; }
    if ! generate_idl "$WORK/out/$id.ast.json" "$WORK/logs/$id.ast.log"; then
      echo "  AST generation FAILED:"; tail -20 "$WORK/logs/$id.ast.log"
      summary+=("$id: AST FAILED"); failures=$((failures+1)); continue
    fi
    echo "  ast: ok"
    [ $ast_only -eq 1 ] && { summary+=("$id: ast ok"); continue; }

    materialize "$tree" "$rep" ib || { echo "  setup failed"; summary+=("$id: SETUP FAILED"); failures=$((failures+1)); continue; }
    if ! generate_idl "$WORK/out/$id.ib.json" "$WORK/logs/$id.ib.log"; then
      echo "  idl-build generation FAILED:"; grep -E "^(error|Error)" -A8 "$WORK/logs/$id.ib.log" | head -40
      summary+=("$id: IDL-BUILD FAILED (see .work/logs/$id.ib.log)"); failures=$((failures+1)); continue
    fi
    echo "  idl-build: ok"

    if python3 "$HERE/diff_idl.py" "$WORK/out/$id.ast.json" "$WORK/out/$id.ib.json" | tee "$WORK/out/$id.diff.txt"; then
      summary+=("$id: identical (modulo known gaps)")
    else
      summary+=("$id: DIFFERENCES (see .work/out/$id.diff.txt)"); failures=$((failures+1))
    fi
  done
done

# The IDL describes the program, never the backend it is built for: one source built in every
# representation must give identical IDLs, from both generators, apart from the per-crate program
# address and metadata. Only trees whose source compiles in all four representations take part.
if [ $check_only -eq 0 ]; then
  for tree in "${run_trees[@]}"; do
    reps=(${REPS_OF[$tree]})
    [ ${#reps[@]} -ge 4 ] || continue
    for kind in ast ib; do
      first=""
      for rep in "${reps[@]}"; do
        file="$WORK/out/${tree}_${rep}.$kind.json"
        [ -f "$file" ] || continue
        if [ -z "$first" ]; then first="$file"; continue; fi
        if python3 - "$first" "$file" <<'PY'
import json, sys
def load(path):
    idl = json.load(open(path, encoding="utf-8"))
    idl.pop("address", None)
    idl.pop("metadata", None)
    return idl
sys.exit(0 if load(sys.argv[1]) == load(sys.argv[2]) else 1)
PY
        then
          summary+=("${tree}: $kind IDL identical in $(basename "$first" .$kind.json | sed "s/^${tree}_//") and $rep")
        else
          echo "=== ${tree}: $kind IDL differs between representations"
          diff <(python3 -m json.tool "$first" | grep -v '"address"\|"name": "idlp_\|"description": "idlp_') \
               <(python3 -m json.tool "$file" | grep -v '"address"\|"name": "idlp_\|"description": "idlp_') | head -40
          summary+=("${tree}: $kind IDL DIFFERS between $(basename "$first" .$kind.json | sed "s/^${tree}_//") and $rep")
          failures=$((failures+1))
        fi
      done
    done
  done
fi

echo
echo "=== summary"
for line in "${summary[@]}"; do echo "  $line"; done
[ $failures -eq 0 ]
