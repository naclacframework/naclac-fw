#!/usr/bin/env bash
# Materializes a test suite's per-mode program crates from its single shared source tree.
#
#   scripts/gen-variants.sh tests/<suite>
#
# The suite keeps one canonical program under <suite>/variants/:
#   variants.conf      description, optional lang_features, client (the canonical
#                      client crate name the tests use), optional module (the
#                      canonical `#[program]` module name, renamed to each
#                      program's own name), then one
#                      `program | runtime | layout [| family]` line per mode
#   src/               the program source, identical in every mode
#   tests/*.rs         the integration tests; every occurrence of the canonical
#                      client crate name is renamed to the mode's own client crate
# `idl_build=true` declares the `idl-build` feature in every generated manifest,
# for suites whose source or tests use `cfg(feature = "idl-build")`; the CLI then
# generates their IDL by compiling the program instead of walking its source.
# `from.<family>=<dir>` makes a family reuse another source directory (`.` is the
# variants root) so it can differ from it only by rewrite rules.
# `base.<family>=<name>` names the program whose name has no mode suffix; the rest
# of each program's name is its suffix. `rewrite.<family>=<sed -E expression>`
# lines are applied to the family's copied .rs files. `overlay.<family>=<dir>` copies
# <variants>/<dir>/src over the family's copied src (runtime-specific replacements),
# and `remove.<family>=<path>` deletes a file, relative to the program crate, after
# copying. Rewrite expressions and
# the dependency fragments below may use %NAME%, %SUFFIX% (e.g. `_zc`),
# %KEBAB_SUFFIX% (`-zc`), %CAMEL_SUFFIX% (`Zc`), %RUNTIME% and %LAYOUT%.
# A family directory may also hold `dependencies.toml` and `dev-dependencies.toml`,
# whose lines are appended to the generated [dependencies] / [dev-dependencies]
# tables, and `dependencies.<runtime>.toml` / `dev-dependencies.<runtime>.toml`
# appended only for programs on that runtime. `<suite>/variants/static/<program>/` holds plain programs that are copied
# into programs/ unchanged (for example a hook program a test loads).
# A suite whose modes need different sources (for example zero-copy and heap
# layouts) keeps one directory per family, <suite>/variants/<family>/{src,tests},
# names it in the optional fourth column, and gives that family its own
# `client.<family>=` and `module.<family>=` keys.
# and this script writes <suite>/programs/<name>/ for every mode plus the suite's
# Naclac.toml. Both are generated and git-ignored; run this before `naclac build`
# in the suite directory. `declare_id!` in the copied source is a placeholder that
# `naclac build` replaces with the program's own keypair address.
#
# runtime: pinocchio | solana        layout: bytemuck | borsh | wincode
# (borsh is available on the solana runtime only)

set -euo pipefail

suite_dir=$(cd "${1:?usage: gen-variants.sh <suite dir>}" && pwd)
variants_dir="$suite_dir/variants"
conf="$variants_dir/variants.conf"
[ -f "$conf" ] || { echo "gen-variants: missing $conf" >&2; exit 1; }

placeholder_id="11111111111111111111111111111111"
description=""
lang_features=""
idl_build=""
declare -A family_client=()
declare -A family_module=()
declare -A family_base=()
declare -A family_from=()
declare -A family_rewrites=()
declare -A family_overlay=()
declare -A family_removes=()

camel() {
  local out="" part
  IFS='_' read -ra parts <<<"$1"
  for part in "${parts[@]}"; do
    [ -z "$part" ] || out+="${part^}"
  done
  printf '%s' "$out"
}
names=()
runtimes=()
layouts=()
families=()

while IFS= read -r line || [ -n "$line" ]; do
  line=${line%$'\r'}
  case $line in
    ''|'#'*) continue ;;
    description=*) description=${line#description=} ;;
    lang_features=*) lang_features=${line#lang_features=} ;;
    idl_build=*) idl_build=${line#idl_build=} ;;
    client=*) family_client[_]=${line#client=} ;;
    client.*=*) key=${line#client.}; family_client["${key%%=*}"]=${key#*=} ;;
    module=*) family_module[_]=${line#module=} ;;
    from.*=*) key=${line#from.}; family_from["${key%%=*}"]=${key#*=} ;;
    base.*=*) key=${line#base.}; family_base["${key%%=*}"]=${key#*=} ;;
    overlay.*=*) key=${line#overlay.}; family_overlay["${key%%=*}"]=${key#*=} ;;
    remove.*=*) key=${line#remove.}; family_removes["${key%%=*}"]+="${key#*=}"$'\n' ;;
    rewrite.*=*) key=${line#rewrite.}; family_rewrites["${key%%=*}"]+="${key#*=}"$'\n' ;;
    module.*=*) key=${line#module.}; family_module["${key%%=*}"]=${key#*=} ;;
    *'|'*)
      IFS='|' read -r name runtime layout family <<<"$line"
      names+=("${name// /}")
      runtimes+=("${runtime// /}")
      layouts+=("${layout// /}")
      families+=("${family// /}")
      ;;
    *) echo "gen-variants: cannot parse line: $line" >&2; exit 1 ;;
  esac
done <"$conf"

[ ${#names[@]} -gt 0 ] || { echo "gen-variants: $conf lists no programs" >&2; exit 1; }

previous_naclac="$suite_dir/Naclac.toml"
declare -A known_ids=()
if [ -f "$previous_naclac" ]; then
  while IFS= read -r entry; do
    key=${entry%% =*}
    value=${entry#*\"}
    known_ids["$key"]=${value%\"*}
  done < <(grep -E '^[A-Za-z0-9_]+ = "[1-9A-HJ-NP-Za-km-z]{32,44}"' "$previous_naclac" | tr -d '\r' || true)
fi

lang_feature_list=""
if [ -n "$lang_features" ]; then
  IFS=',' read -ra requested <<<"$lang_features"
  for feature in "${requested[@]}"; do
    lang_feature_list+="${lang_feature_list:+, }\"${feature// /}\""
  done
fi

rm -rf "$suite_dir/programs"
mkdir -p "$suite_dir/programs"
if [ -d "$variants_dir/static" ]; then
  cp -r "$variants_dir"/static/* "$suite_dir/programs/"
fi

{
  cat <<'EOF'
[toolchain]
package_manager = "yarn"

[features]
resolution = true
skip-lint = false

[programs.localnet]
EOF
} >"$suite_dir/Naclac.toml.new"

for i in "${!names[@]}"; do
  name=${names[$i]}
  runtime=${runtimes[$i]}
  layout=${layouts[$i]}

  case $layout in bytemuck|borsh|wincode) ;; *) echo "gen-variants: $name: unknown layout '$layout'" >&2; exit 1 ;; esac
  case $runtime in
    pinocchio)
      [ "$layout" != borsh ] || { echo "gen-variants: $name: borsh needs the solana runtime" >&2; exit 1; }
      default_list="\"$layout\""
      feature_lines="$layout = [\"naclac-lang/$layout\"]"
      default_features_off=""
      ;;
    solana)
      default_list="\"solana\", \"$layout\""
      feature_lines="solana = [\"naclac-lang/solana\"]"$'\n'"$layout = [\"naclac-lang/$layout\"]"
      default_features_off=", default-features = false"
      ;;
    *) echo "gen-variants: $name: unknown runtime '$runtime'" >&2; exit 1 ;;
  esac

  lang_dep_features=""
  [ -z "$lang_feature_list" ] || lang_dep_features=", features = [$lang_feature_list]"
  if [ "$idl_build" = true ]; then
    feature_lines+=$'\n'"idl-build = [\"naclac-lang/idl-build\"]"
  fi

  family=${families[$i]}
  source_dir="$variants_dir${family:+/$family}"
  source_from=${family_from[${family:-_}]:-}
  if [ -n "$source_from" ]; then
    if [ "$source_from" = . ]; then source_dir="$variants_dir"; else source_dir="$variants_dir/$source_from"; fi
  fi
  [ -d "$source_dir/src" ] || { echo "gen-variants: $name: missing $source_dir/src" >&2; exit 1; }
  canonical_client=${family_client[${family:-_}]:-}
  if [ -d "$source_dir/tests" ] && [ -z "$canonical_client" ]; then
    echo "gen-variants: $name: no client name for family '${family:-default}'" >&2
    exit 1
  fi
  canonical_module=${family_module[${family:-_}]:-}

  program_dir="$suite_dir/programs/$name"
  mkdir -p "$program_dir/tests"
  cp -r "$source_dir/src" "$program_dir/src"
  overlay_dir=${family_overlay[${family:-_}]:-}
  if [ -n "$overlay_dir" ]; then
    cp -r "$variants_dir/$overlay_dir/src/." "$program_dir/src/"
  fi
  if [ -n "$canonical_module" ]; then
    sed -i "s/^pub mod ${canonical_module} {/pub mod ${name} {/" "$program_dir/src/lib.rs"
  fi
  if [ -d "$source_dir/tests" ]; then
    for test_path in "$source_dir"/tests/*.rs; do
      sed "s/\\b${canonical_client}\\b/${name}_client/g" "$test_path" \
        >"$program_dir/tests/$(basename "$test_path")"
    done
  fi

  while IFS= read -r removed_path; do
    [ -n "$removed_path" ] || continue
    rm -f "$program_dir/$removed_path"
  done <<<"${family_removes[${family:-_}]:-}"

  client_dep="${name//_/-}-client"
  base_name=${family_base[${family:-_}]:-}
  suffix=${name#"$base_name"}
  camel_suffix=$(camel "$suffix")
  expand() {
    local text=$1
    text=${text//%SUFFIX%/$suffix}
    text=${text//%KEBAB_SUFFIX%/${suffix//_/-}}
    text=${text//%CAMEL_SUFFIX%/$camel_suffix}
    text=${text//%RUNTIME%/$runtime}
    text=${text//%LAYOUT%/$layout}
    text=${text//%NAME%/$name}
    printf '%s' "$text"
  }

  while IFS= read -r rewrite_expr; do
    [ -n "$rewrite_expr" ] || continue
    find "$program_dir/src" "$program_dir/tests" -name '*.rs' 2>/dev/null \
      -exec sed -i -E "$(expand "$rewrite_expr")" {} +
  done <<<"${family_rewrites[${family:-_}]:-}"

  extra_dependencies=""
  extra_dev_dependencies=""
  [ ! -f "$source_dir/dependencies.toml" ] || extra_dependencies=$'\n'"$(expand "$(tr -d '\r' <"$source_dir/dependencies.toml")")"
  [ ! -f "$source_dir/dev-dependencies.toml" ] || extra_dev_dependencies=$'\n'"$(expand "$(tr -d '\r' <"$source_dir/dev-dependencies.toml")")"
  [ ! -f "$source_dir/dependencies.$runtime.toml" ] || extra_dependencies+=$'\n'"$(expand "$(tr -d '\r' <"$source_dir/dependencies.$runtime.toml")")"
  [ ! -f "$source_dir/dev-dependencies.$runtime.toml" ] || extra_dev_dependencies+=$'\n'"$(expand "$(tr -d '\r' <"$source_dir/dev-dependencies.$runtime.toml")")"

  cat >"$program_dir/Cargo.toml" <<EOF
[package]
name = "$name"
version = "0.1.0"
description = "$description ($runtime, $layout)"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[features]
no-entrypoint = []
cpi = ["no-entrypoint"]
custom-heap = []
custom-panic = []
debug-mode = []
default = [$default_list]
$feature_lines

[dependencies]
naclac-lang = { version = "0.1.0", path = "../../../../lang"$default_features_off$lang_dep_features }$extra_dependencies

[dev-dependencies]
naclac-client = { version = "0.1.0", path = "../../../../client-rs" }
$client_dep = { version = "0.1.0", path = "../../artifacts/clients/rust/$name" }$extra_dev_dependencies

[lints]
workspace = true
EOF

  echo "$name = \"${known_ids[$name]:-$placeholder_id}\"" >>"$suite_dir/Naclac.toml.new"
  echo "gen-variants: $name ($runtime, $layout)"
done

cat >>"$suite_dir/Naclac.toml.new" <<'EOF'

[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"

[scripts]
test-rust = "cargo test -- --nocapture"
EOF
mv "$suite_dir/Naclac.toml.new" "$suite_dir/Naclac.toml"
