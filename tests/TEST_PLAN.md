# naclac-fw test plan

Maps out what a `tests/` directory (framework regression tests, distinct from
`examples/` — see the folder-purpose discussion this doc follows from) needs
to cover. This is not about re-verifying anything in the audit/plan docs at
repo root — those are already fixed and confirmed. This is the ground-up
coverage map for the framework's actual current surface.

## The mode matrix — read this first

Everything in this repo runs in one of **three build configurations**, and
every test case below needs to state which of the three it targets. This is
the "one backend split into two" the framework has:

| # | Name used below | How it's selected | Account wrapper | Serialization |
|---|---|---|---|---|
| 1 | **solana-borsh** | default `solana` backend + `borsh` feature on | `Account<T>` (real struct, `.data` field) | Borsh, heap-allocated |
| 2 | **solana-zerocopy** | default `solana` backend, `borsh` feature **off** | `Account<T>` zero-copy branch | `Pod`/`bytemuck`, in-place |
| 3 | **pinocchio** | `pinocchio` feature on (implies zero-copy, no_std) | `Account<T>` zero-copy branch | `Pod`/`bytemuck`, in-place |

So "solana" is one backend (`solana-program`-based, has a real `AccountInfo`
struct with `Rc<RefCell<...>>` lamports/data) that itself splits into two
serialization modes (borsh vs zero-copy), and "pinocchio" is the second,
wholly separate backend (no_std, always zero-copy, different `AccountInfo`
shape entirely, different CPI mechanism). Any test that touches account
data shape, discriminators, or CPI needs to run in all three; anything that's
purely about macro *syntax acceptance* (does this attribute parse) only
needs to run once, in whichever mode is cheapest to compile.

Each `tests/<case>/` directory is its own standalone Cargo workspace (same
pattern as `examples/*/Cargo.toml` — `[workspace]` + `members = ["programs/*"]`),
never a member of the root workspace, and needs its own `Naclac.toml`
registering each program under `[programs.localnet]`. Where a case needs to
run in more than one mode, give it three sub-crates named `<name>` (pinocchio
— the framework's default, no suffix), `<name>_borsh`, `<name>_zc`, matching
`examples/counter`'s exact naming convention.

**The real harness — proven via `tests/discriminator/`, don't hand-roll a
litesvm harness.** naclac already has one, in `naclac-client`
(`NaclacProvider::new("litesvm", payer)`, `.add_program(...)`,
`.send_and_confirm()`, `.parse_events_zero_copy()`/`.parse_events_borsh()`).
The actual per-case workflow:

1. Write the fixture program(s) under `programs/<name>/src/...`.
2. `naclac build` — compiles every program in the workspace to
   `target/deploy/<name>.so` and (via a pre-build stub pass) ensures
   `clients/rust/<name>/` exists before `cargo build-sbf`'s internal
   `cargo metadata` call, which resolves the whole workspace including
   dev-dependencies.
3. `naclac generate` — overwrites the stub with the real generated client
   (`clients/rust/<name>/`, committed to git — confirmed via
   `examples/counter`'s `.gitignore`, which excludes `target/` but not
   `clients/`).
4. `programs/<name>/tests/<name>_test.rs` — a `#[test]` using
   `naclac_client::NaclacProvider` + the generated client's typed
   `build_<instruction>()`/`<Instruction>Accounts`/`fetch_<component>()`/
   `get_<component>_pda()` functions (not `get_<component>_account_pda` —
   that was an early naming guess that turned out wrong; always verify
   against the real generated output in `clients/rust/<name>/src/`, don't
   assume the naming convention holds without checking).

**Critical gotcha: never `cargo test --workspace` across a case's sibling
mode variants.** Sibling packages in one case (e.g. `discriminator` /
`discriminator_borsh` / `discriminator_zc`) request mutually incompatible
`naclac-lang` features (`pinocchio` vs `borsh` vs plain default) from the
*same* dependency. `--workspace` unifies features across the whole resolve,
silently building `naclac-lang` with `pinocchio` *and* `borsh` *and* default
all switched on at once — this doesn't error, it just corrupts every mode
simultaneously (missing `BorshSerialize`, wrong `AccountInfo` shape, etc.).
Always test one package at a time: `cargo test -p <name>`. Use
`scripts/test-all.sh` (repo root) to run every case's every package
correctly — it's the automation layer for this, not something to redo by
hand per case.

**Native-fs target dir: handled by `scripts/test-all.sh` itself, don't add
a per-case `.cargo/config.toml` for this.** `cargo test` on a Windows-mounted
drive under WSL intermittently fails with `Permission denied` writing
`.rmeta` files (NTFS-via-9p locking, not a naclac bug) unless build output
goes to a native Linux path. The fix is a `CARGO_TARGET_DIR` env var set
*only* around the script's own `cargo test` calls — **not** a
`.cargo/config.toml` in the case directory. A `.cargo/config.toml` there
applies to *every* cargo invocation in that tree, including `naclac
build`'s internal `cargo build-sbf` — which silently relocates
`target/deploy/*.so` off the relative path every test file's
`load_program()` helper expects, breaking `provider.add_program(...)` with
a confusing `NotFound` at test time (this actually happened once, on
`tests/pda-seeds/` — cost a full rebuild cycle to track down). Keep `naclac
build`/`naclac generate` using the plain default target dir; only
`scripts/test-all.sh`'s `cargo test` invocations get redirected. (`naclac-core/.cargo/config.toml`
predates this lesson and is a different situation — that's a workspace
member of the *root* build, not a `tests/*` case — leave it as-is, just
don't copy its pattern into `tests/*/`.)

---

## 0. `tests/single-file-layout/` — source discovery doesn't require the folder split ✅ DONE

**Modes:** one is sufficient (this tests `naclac-syn`'s source discovery, a
build-time/macro-expansion-time property that's identical across all three
modes — not runtime account/serialization behavior).

Every example and every other test case in this plan splits a program into
`components/`, `instructions/`, etc. That's a convention this repo follows,
not something the macros or `naclac-syn` require — confirmed directly from
`naclac-syn/src/lib.rs`'s `parse_workspace_program`: it recursively
`fs::read_dir`s every file under `src/` and parses each one for
`#[component]`/`#[derive(Accounts)]`/`#[instruction]` items, with no
assumption about *which* files those live in. `tests/single-file-layout/`
proves this end-to-end rather than leaving it as an unverified read of the
code: one `Counter` component, two `#[derive(Accounts)]` structs, and both
instruction handlers all live directly in one `lib.rs`. The test exercises
the full pipeline — IDL generation, generated client SDK (including the
`get_counter_pda` PDA helper, which depends on the IDL correctly resolving
`seeds`/`bump` off the component), and real on-chain execution — and
confirms all of it works identically to the split layout.

---

## 1. `tests/discriminator/` — account type confusion ✅ DONE

**Modes:** all three.

The exact scenario the now-fixed audit finding #1 describes, kept as a
permanent regression test rather than a one-time verification: define two
`#[component]` types owned by the same program (`Vault`, `Config`), write an
instruction (`read_vault`) expecting `Account<Vault>` with no `seeds`/`address`
constraint on that field (so the discriminator check is the *only* thing
that can reject a wrong-type account), and confirm passing an initialized
`Config` account in that slot is rejected instead of silently deserializing
garbage. A positive-path companion test confirms a real `Vault` account is
still accepted, so the negative test can't pass for the wrong reason.

- `programs/discriminator` — pinocchio (framework default)
- `programs/discriminator_borsh` — solana-borsh
- `programs/discriminator_zc` — solana-zerocopy

All 6 tests (2 per mode) pass. Verified via `scripts/test-all.sh discriminator`.

**Note:** this is the single highest-value test in the whole plan — it's a
security property (type confusion), not a convenience feature, and it's
exactly the kind of thing that regresses silently if `Account::try_from`
ever gets refactored again.

---

## 2. `tests/accounts-constraints/` — every `#[account(...)]` constraint, individually ✅ DONE

**Modes:** all three (constraint codegen is macro-level and mode-aware, per
`security.rs`'s `is_zero_copy_account_field` branching).

One instruction per constraint keyword found in `naclac-macros/src/instruction/parser.rs`
and `security.rs`, each with a positive case (constraint satisfied) and a
negative case (constraint violated → expect the specific error, not just
"any error"):

**Correction to an earlier draft of this plan:** there is no literal
`has_one` or `constraint = <expr>` keyword. `naclac-macros/src/instruction/parser.rs`'s
`is_reserved` list treats any *non-reserved* bare identifier `X = Y` as a
generic relation: "check `self.X` (a data field on the annotated account)
equals `accounts.Y`'s address" — this is the `has_one`-equivalent, spelled
with the real field name instead of a fixed keyword (e.g. `admin =
authority`, not `has_one = authority`). There's no generic boolean-expression
escape hatch (`constraint = <expr>`) — the relation codegen always calls
`.address()` on the target, so it only ever compares two account keys, never
an arbitrary expression.

**✅ Core subset built and passing (9/9, all 3 modes).**
`tests/accounts-constraints/` exists (3 modes: `accounts_constraints`,
`accounts_constraints_borsh`, `accounts_constraints_zc`) with one program
covering:

| Constraint | Instruction | What's asserted |
|---|---|---|
| `init` + `payer` + `space` | `init_vault` | account created, discriminator written, rent-exempt, explicit `space = 8 + size_of::<Vault>()` |
| `init_if_needed` | `init_if_needed_ledger` | first call creates+sets value; second call with a different arg does not overwrite it |
| `signer` | `require_signer` | missing signature rejected; real signature accepted |
| `owner` | `check_owner` | account owned by the wrong program rejected; System-Program-owned account accepted |
| `address` | `check_address` | account whose own key doesn't match the expected constant rejected |
| `mut` | `touch_mut_vault` | write with a correctly-writable account succeeds; a hand-flipped non-writable `AccountMeta` on the same built instruction is rejected |
| relation (`has_one`-equivalent) | `related_vault` (`admin = authority`) | mismatched `authority` rejected; matching one accepted |
| `seeds` + explicit `bump = <expr>` | `init_seeded` / `touch_seeded` | correct stored bump accepted; wrong bump rejected (`ConstraintSeeds`) |
| `close` | `close_vault` | lamports drained to destination; account can't be loaded as the same type again afterward |

**`executable` — ✅ built and passing (10/10, all 3 modes:
`accounts_constraints`, `accounts_constraints_borsh`,
`accounts_constraints_zc`).** Added `check_executable` — a single
`#[account(executable)]`-constrained bare `AccountInfo` field, deliberately
*not* `Program<T>` (which already enforces executable-ness internally on
its own, unconditionally — see `naclac-core/src/wrappers/program.rs`'s
`try_from`; `executable` on a plain `AccountInfo` is for asserting the same
property without that wrapper). Positive case passes the real System
Program account (genuinely executable); negative case passes the payer's
own signer account (not executable), expecting
`NaclacError::ConstraintExecutable`. Files: `programs/*/src/instructions/check_executable.rs`,
registered in each variant's `instructions/mod.rs`/`lib.rs`,
`clients/rust/*/src/instructions/check_executable.rs`, and a new
`executable_constraint_rejects_non_executable_account` test in each of the
3 `tests/*.rs` files.

**`rent_exempt`, `close`'s self-close guard, the relation `@ CustomError`
syntax, and `seeds::program` — ✅ built and passing (14/14, all 3 modes:
`accounts_constraints`, `accounts_constraints_borsh`,
`accounts_constraints_zc`).** Four more constraint paths that had zero test
coverage:

- **`rent_exempt`** (a brand-new, bare-flag constraint, no prior coverage at
  all) — `check_rent_exempt`, a single `#[account(rent_exempt)]`-constrained
  bare `AccountInfo` field (`security.rs`'s "Rent-Exemption Check": non-pinocchio
  calls `Rent::get()?.is_exempt(lamports, data_len)`; pinocchio uses a
  const-rent formula). Positive case passes `provider.payer.address()`
  (litesvm funds the default payer with a huge SOL balance, trivially
  rent-exempt for a 0-byte account); negative case airdrops a fresh keypair
  only 1,000 lamports (via `NaclacProvider::airdrop`, a direct lamport
  credit — no need for a full funding transaction) and expects
  `NaclacError::ConstraintRentExempt`. `CheckRentExempt { target }` — field
  index 0 -> `3000 + 0*100 + ConstraintRentExempt(5) = 3005`
  (discriminant read directly from `naclac-core/src/error.rs`).
- **`close`'s self-close guard** — `close_account.rs` rejects
  `target == dest` unconditionally, before any owner/data check, but every
  existing `close` test (`close_vault`) only ever exercised the success path
  with a distinct destination. New instruction `close_vault_self`: two bare
  `AccountInfo` fields, `target` (`#[account(mut, close = destination,
  unsafe(alias))]`) and `destination` (`#[account(mut, unsafe(alias))]`).
  `unsafe(alias)` on both is load-bearing, not decorative: without it,
  passing the same address into two declared `mut` slots is caught by a
  *different* guard first — `accounts.rs`'s `__duplicates` bitvec
  intersecting `MUT_MASK` — with `ConstraintDuplicateMutableAccount`, before
  `teardown`'s close logic ever runs. Marking both fields `unsafe(alias)`
  excludes them from `MUT_MASK` (confirmed by reading `accounts.rs`'s
  `mut_mask_steps`: `if is_mut && !field.is_alias`), so the transaction
  actually reaches `close_account.rs`'s own explicit self-check.
  `CloseVaultSelf { target, destination }` — field index 0 ->
  `3000 + 0*100 + ConstraintClose(26) = 3026`.
- **Relation constraint custom error (`field @ CustomError` syntax)** — never
  exercised anywhere in this repo; every existing relation test
  (`related_vault`) only takes the default-error path
  (`NaclacError::Unauthorized`). Added `programs/*/src/errors.rs`
  (`#[error_code] pub enum VaultError { WrongAdmin }`, registered as
  `pub mod errors;` in each variant's `lib.rs`) and a new instruction
  `related_vault_custom_error`, identical to `related_vault` except
  `#[account(mut, admin = authority @ VaultError::WrongAdmin)]`. Mismatched
  case now gets `VaultError::WrongAdmin`'s own code instead of the default
  `NaclacError::Unauthorized` — `#[error_code]` offsets custom discriminants
  by 6000 (`naclac-macros/src/error_code.rs`), so with no explicit
  discriminant `WrongAdmin` (the enum's only variant) lands at exactly 6000,
  a genuinely different codespace from `related_vault`'s own mismatch case
  (3021), proving the custom-error path fired instead of the default rather
  than just "some error fired."
- **`seeds::program`** (external-program PDA derivation, parsed in
  `parser.rs`, consumed in `security.rs`'s PDA derivation as
  `pda_program_tokens`) — new instruction `check_external_pda`, a bare
  `AccountInfo` field with `#[account(seeds = [SEED_EXTERNAL_PDA], bump =
  bump, seeds::program = naclac_lang::prelude::SYSTEM_PROGRAM_ID)]` (explicit
  `bump = bump`, not bare `bump`: a bare `AccountInfo` has no stored `.bump`
  field for the auto-bump path to read — same reasoning as
  `touch_seeded.rs`). Uses the real System Program ID as the "external"
  program — real, stable, no separate deployment needed. Positive case
  derives the PDA off-chain via `Address::find_program_address(&[b"external_pda"],
  &SYSTEM_PROGRAM_ID)` and confirms the on-chain program accepts it with the
  correct bump; negative case passes a deliberately wrong bump and expects
  `NaclacError::ConstraintSeeds`. `CheckExternalPda { target }` — field
  index 0 -> `3000 + 0*100 + ConstraintSeeds(6) = 3006`.

**Real framework bug found and fixed while adding `seeds::program`
coverage** (in `naclac-macros/src/instruction/security.rs`'s PDA-validation
codegen, not a test-authoring mistake): the compile-time-precomputed-PDA
optimization (`is_pda_precomputed`, taken whenever every seed element is a
literal) always derived the expected PDA against the *current* crate's own
program ID (via `find_program_id`/`Naclac.toml`), completely ignoring
`field.pda_program` — the `seeds::program = X` override. Combined with an
all-literal seed array (exactly `check_external_pda`'s shape:
`seeds = [SEED_EXTERNAL_PDA]`, a single literal-byte-string seed), this
would have silently computed the wrong `expected_pda` (derived against our
own program instead of the System Program), making a correctly-derived
external-program PDA always fail the address check. Fixed by skipping the
precompute path whenever `field.pda_program.is_some()`, forcing the dynamic
hash-and-compare path instead — which already correctly threads
`pda_program_tokens` through the hash inputs. Without this fix,
`seeds::program` combined with a literal seed array would never have worked
at all, regardless of what the test asserted.

**Verified by a real build: all four pass (14/14, all 3 modes).** The
`unsafe(alias)` reasoning for `close_vault_self` and the `seeds::program`
fix in `security.rs` both held up under an actual `naclac build` +
`scripts/test-all.sh accounts-constraints` run. One more real bug turned up
along the way, unrelated to any of the four: `rent_exempt_constraint_rejects_underfunded_account`
used `NaclacProvider::airdrop` to fund a test account with a deliberately
tiny, sub-rent-exempt balance — but `airdrop` is a real System Program
transfer, and the runtime rejects any transfer that would leave a new
account underfunded, so the test never even reached the constraint it was
trying to exercise. Added `NaclacProvider::set_account_lamports`
(`naclac-client/src/provider.rs`), which writes the lamport balance
directly via litesvm's `set_account` (litesvm-only, bypasses transaction/rent
enforcement entirely), and switched the test to use it.

**Real bug found and fixed while verifying this**: `touch_seeded.rs` (all 3
variants, pre-existing, unrelated to `executable`) never imported
`SEED_SEEDED` at all (`use crate::constants::SEED_SEEDED;` missing — its
sibling `init_seeded.rs` has it) — a plain missing-import bug that had
apparently never been caught by an actual build of this exact file.

`token::mint` / `token::authority` / `token::program` and
`mint::decimals` / `mint::authority` / `mint::freeze_authority` were
deferred to `tests/token/` (section 6 below), where `token::program` and
`mint::freeze_authority` are now built and passing too (`decimals`'s
generic `Unauthorized` and `token::mint`/`token::authority`'s
`ConstraintAccountIsNone`/`ConstraintAddress` split were already covered
there).

**Note:** don't build 20 separate example programs for this — one program
per mode with ~20 instructions (one per constraint) keeps compile time sane
while still isolating failures per constraint.

---

## 3. `tests/pda-seeds/` — the seed-expression classification bug class ✅ DONE

**Modes:** solana-zerocopy and pinocchio only (this is specifically about
`Account<T>`'s zero-copy-branch `Deref` rewrite path; solana-borsh doesn't
hit this code path at all).

This is the area the audit found live bugs in twice (#3, and Part 6 of the
consistency plan) — the kind of code that's proven itself prone to silent
regressions. Five instructions (`init_registry`, `init_entry`,
`touch_entry_bare_bump`, `touch_registry_explicit_bump`, `init_child`) cover:

- Literal seed + bare bump on `init` (compile-time-precomputed PDA path —
  `security.rs`'s `resolve_seed_literal` textually reads `constants.rs` off
  disk at macro-expansion time and embeds the resolved bytes directly.
  **Correction:** the seed constant's `use` import used to show as unused in
  the expanded output — noted here as "expected", not a false-positive. That
  was true at the time, but the underlying cause (the expanded code never
  referenced the symbol, only its file-scraped byte value) was itself a real
  gap: proc-macros can't ask the compiler what an identifier resolves to
  (they run before name resolution), so the byte-value lookup has to stay a
  file read — but nothing required the *generated code* to skip referencing
  the symbol too. Fixed in `naclac-macros/src/instruction/security.rs`'s
  precomputed-PDA branch: it now splices the user's original seed
  expressions back in as a harmless `let _ = &SEED_COUNTER;`-style
  reference, so rustc's real post-expansion name resolution sees the import
  as used. The warning should no longer appear for this shape.)
- Dynamic seed via whole-account `.as_ref()` on `init` — the exact shape
  audit finding #3 broke on, also exercising `init_cpi.rs`'s independent
  seed-binding codegen
- Dynamic seed + bare bump on an *existing* (non-`init`) account — the
  confirmed zero-coverage gap from Part 6e, now covered
- Explicit `bump = <expr>`, positive case + a deliberately-wrong-bump
  negative case
- Nested field-access + method-chain seed (`registry.bump.to_le_bytes().as_ref()`)

Not covered (lower priority, left as a follow-up): `.to_be_bytes()`/`.as_bytes()`
on a literal/string seed specifically. All cases pass in both modes —
verified via `scripts/test-all.sh pda-seeds`.

---

## 4. `tests/dup-mut/` — duplicate mutable account aliasing ✅ DONE

**✅ Built and passing (6/6 `dup_mut`, 7/7 `dup_mut_borsh`, 7/7 `dup_mut_zc`).**
`tests/dup-mut/` exists (3 modes). Beyond the original bullets below, also
covers a 3-mut-slot partial-aliasing case (`touch_triple_partial_alias`,
`init_vault_c`), a `Box<Account<T>>` field (`dup_mut_borsh`'s
`touch_boxed_vault`), and a real `ZcString` instruction-arg round trip
(`dup_mut_zc`'s `write_note`/`Note`).

Two real bugs found and fixed along the way:
- `naclac-syn/src/parser.rs`'s `rust_type_to_idl` had no case for
  `ZcString`, so it fell through to `{"defined": "ZcString"}` (treated as a
  user-defined type) instead of `"string"`. Broke both the Rust and TS
  client generators, which then tried to reference a `types::ZcString` that
  never existed. Fixed by mapping `ZcString` alongside `String`.
- The Rust client generator's zero-copy `IxArgs` struct always assumed
  every instruction arg was fixed-size (`Copy` + `repr(C, packed)` +
  `bytemuck::Pod`), which can't work for a `String`/`Vec` arg. Fixed in
  `naclac-client-gen/src/rust`: args are now classified pod vs. dynamic:
  pod-only instructions keep the bytemuck fast path, mixed/dynamic ones get
  a plain struct with per-field wire encoding (pod fields raw, dynamic
  fields length-prefixed) matching `accounts.rs`'s own on-chain read order.

One test-expectation bug (not a framework bug): the `remaining_accounts`
duplicate case expected an index-based error code, but the global
`__duplicates` prescan (`program.rs`) already catches any declared-field
collision first and always reports index 0 — the index-based path is
unreachable for that scenario.

**Modes:** all three (the guard lives in `accounts.rs`'s generated
`load_and_validate`, which is mode-agnostic — but verify the `__duplicates`
bitvec actually gets threaded through correctly in all three, since it's
passed as `Option<&AccountBitvec>` and could plausibly be `None` in a code
path specific to one mode).

**Correction to an earlier draft of this plan:** naclac already has this
mechanism — it isn't a gap. Confirmed in
[naclac-macros/src/accounts.rs:426-497](../naclac-macros/src/accounts.rs) and
[instruction/parser.rs:108-117](../naclac-macros/src/instruction/parser.rs):
a compile-time `MUT_MASK` bitmask over every `mut` field is checked against a
runtime `__duplicates` bitvec, *and* a separate pairwise address check runs
against `remaining_accounts`. Real usage confirmed in
`examples/launchpad/launchpad/programs/amm/src/instructions/initialize.rs`:
`#[account(mut, unsafe(alias))]`.

- Two `mut` slots pointing at the same account, no `unsafe(alias)` on either
  → reject with `NaclacError::ConstraintDuplicateMutableAccount`
- Three `mut` slots, every pairwise duplicate position → same rejection
- One `mut` slot marked `unsafe(alias)`, aliased with another `mut` slot →
  accepted (the escape hatch works)
- Bare `alias` (no `unsafe(...)`) → compile error (this one belongs in
  `tests/compile-fail/` below, not here — it's a macro-parse-time rejection,
  not a runtime one)
- A duplicate passed via `remaining_accounts` rather than a named field →
  rejected by the separate pairwise check (`accounts.rs:484-494`)

---

## 5. `tests/compile-fail/` — every macro-level `compile_error!`/`syn::Error` actually fires ✅ DONE (14/15 sites)

**Modes:** one is sufficient per case (this tests macro-expansion-time
rejection, not runtime behavior) — uses `trybuild` (compiles a snippet in a
subprocess, asserts it fails to compile).

**Correction to an earlier draft of this plan:** `trybuild` *requires*
committed `.stderr` snapshot files for `compile_fail` cases — without one it
writes to `wip/*.stderr` and fails the overall suite every run until you
move it into `tests/fail/`. This isn't optional/skippable; it's how the
tool is designed to work, and it's the standard convention across the Rust
ecosystem's proc-macro crates (serde, thiserror, etc. all commit exact
`.stderr` snapshots and re-bless via `TRYBUILD=overwrite cargo test` when a
message deliberately changes). Every `tests/fail/*.rs` here has a committed
`tests/fail/*.stderr` snapshot. Each fixture also starts with
`#![allow(unexpected_cfgs)]` — without it, macro-generated `#[cfg(feature =
"pinocchio")]`/`#[cfg(target_os = "solana")]` code triggers noisy
`unexpected_cfg` warnings (this crate has no `[lints.rust] check-cfg`
registration for those names, unlike real program crates), which would
otherwise get baked into the snapshot as unrelated noise.

Not about re-verifying anything already fixed — about closing the gap where
"the codebase builds" was never actually testing "this specific bad input
gets rejected." Final accurate site count: **15** distinct
`compile_error!`/`syn::Error` emission points (re-grep
`compile_error!\|syn::Error::new` before treating this as exhaustive, in case
more get added). The original sweep found 19; 4 were later deliberately
removed from the framework itself (see below) rather than tested, since the
checks they guarded became pure dead weight, not because they were hard to
test:

| File | Sites |
|---|---|
| `accounts.rs` | 3 — `Pubkey` field, `Pubkey` instruction arg, heap type (`Vec`/`String`) as a zero-copy Accounts field |
| `component.rs` | 2 — `Pubkey` field, heap type in a zero-copy component |
| `event.rs` | 1 — `Pubkey` field |
| `lib.rs` | 1 — banned `find_program_address`/`create_program_address` (text-scanned across `#[instruction]`/`#[program]`/`#[derive(Accounts)]` bodies) |
| `system.rs` | 2 — unparseable `#[system(invariant = "...")]` expression, `f32`/`f64` banned in `#[system]` functions |
| `instruction/parser.rs` | 3 — bare `alias` rejection, `has_one = <dotted.path>` (must be a simple ident), bare `AccountInfo` field missing a `/// SAFETY:` comment |
| `instruction/security.rs` | 3 — `bump = <expr>` combined with `init` on a compile-time-precomputed literal-seed PDA, `seeds = [...]` with no `bump` at all, `has_one = <unknown field>` |

**Removed from the framework, not tested:** the `#[component(...)]`/
`#[event(...)]`/`#[program(...)]`/`#[instruction(...)]` (function attribute)
"no longer takes arguments" rejections that used to live in `component.rs`,
`event.rs`, `program.rs`, `instruction/mod.rs`. These existed only to catch
the old, pre-consistency-plan `zero_copy` argument syntax during the
migration to auto-detected mode. Since mode is now fully auto-detected from
the `borsh` feature regardless of what (if anything) is written in the
attribute's parens, leaving a stray legacy argument there is a genuine
no-op, not a footgun — so with the migration long complete and no external
users yet, the guard was judged pure dead weight and removed outright
rather than kept as permanent migration-era ceremony. If naclac gains
external users before any old tutorials/LLM training data pushes people
toward the old syntax again, this is worth revisiting.

**14 of 15 remaining sites are covered as `tests/fail/*.rs` fixtures.** The remaining one
(`security.rs`'s `bump = <expr>` + `init` + precomputed-PDA check,
`:325-332`) can't be triggered from an isolated `trybuild` fixture: it only
fires inside the `is_pda_precomputed` branch, which requires
`find_program_id` (`security.rs:812`) to successfully read a `Naclac.toml`
walked up from `CARGO_MANIFEST_DIR` — a trybuild fixture has no such file in
its scratch compile directory, so that branch is structurally unreachable
here regardless of what the fixture's seed expression looks like. Testing
it needs a real program workspace with a working `Naclac.toml`/`declare_id!`
instead (e.g. folded into a future case, or its own tiny one) — noted as a
follow-up, not silently dropped.

Where a site has an accompanying *valid* form, a `tests/pass/*.rs`
counterpart exists too, so a future refactor that makes the check *too*
strict (rejecting valid code) gets caught, not just one that makes it too
loose: `unsafe(alias)` (bare-`alias`'s valid form), a real `/// SAFETY: ...`
comment (`AccountInfo`'s valid form), bare `bump` (the missing-bump case's
valid form).

**Note on `.cargo/config.toml` here:** unlike `tests/discriminator/`/`tests/pda-seeds/`,
a per-case native-target-dir redirect *is* safe in `tests/compile-fail/` —
there's no `naclac build`/`cargo build-sbf` involved at all, so there's no
`target/deploy/*.so` path for a redirect to silently break.

---

## 6. `tests/token/` — SPL / pinocchio-token parity ✅ DONE (core subset)

**Modes:** solana-borsh, solana-zerocopy, pinocchio (token backend selection
follows the same 3-way split via the unified `token` feature).

**✅ Core subset built and passing (6/6, all 3 modes).** `tests/token/`
exists (3 modes: `token`, `token_borsh`, `token_zc`), closing out the
`token::*`/`mint::*` constraints deferred from `tests/accounts-constraints/`
(same underlying `security.rs` code, so kept together rather than
duplicated). Two real bugs found and fixed while first running this:
- `token_program: Address::default()` in the test was wrong — that
  zero-address placeholder only ever worked for `system_program` because
  the System Program's real address genuinely *is* all-zero bytes; the
  Token Program's real address is a proper non-zero key, so the on-chain
  `Program<Token>` check correctly rejected the mismatch. Not a framework
  bug — a wrong assumption in the test, generalized from a coincidence.
- `init_mint_authority`'s bare `bump` never got auto-written back into the
  account in solana-borsh mode — confirmed via `accounts.rs`'s
  `mut_zero_copy_fields` gate that the auto-write-back is genuinely
  zero-copy-only, by design (real `Account<T>` in borsh mode isn't
  zero-copy). Fixed by manually writing
  `ctx.accounts.mint_authority.bump = ctx.bumps.mint_authority` in the
  handler — `Context::bumps` (`naclac-core/src/context.rs:45`) is populated
  in every mode, so this is a portable no-op in the two modes where the
  framework already did it automatically. Worth remembering for any future
  case: an instruction relying on bare `bump`'s auto-write-back needs this
  explicit line if it's meant to run in solana-borsh mode too.

- `init_mint_authority` — sets up a PDA (`MintAuthority`) that owns every
  mint/vault this case creates, mirroring `examples/launchpad`'s
  `LaunchRecord` role
- `create_mint` — `mint::decimals` + `mint::authority` combined with `init`
  (real CPI-based mint creation via `init_cpi.rs`, mirroring
  `examples/launchpad/.../create_mint.rs` exactly, bare `AccountInfo` field).
  Uses a dynamic seed component (`id: u64`) rather than a pure literal
  specifically so explicit `bump = mint_bump` is allowed — a pure-literal
  seed + `init` requires bare `bump` instead, confirmed the hard way while
  fixing `resolve_seed_literal`'s file-discovery bug (see section 0)
- `check_vault_constraints` — `token::mint`/`token::authority` enforced on
  an *existing* (non-`init`) token account, mirroring
  `examples/escrow/.../make.rs`'s `vault_token_account` exactly. Confirms
  the two error variants are genuinely different, not just similar:
  wrong mint → `ConstraintAccountIsNone`, wrong authority → `ConstraintAddress`
- `mint_to_vault` / `transfer_tokens` — real `mint_to`/`transfer` CPIs,
  signed by the `mint_authority` PDA's own seeds (mirrors
  `examples/launchpad/.../launch_token.rs`'s `mint_to_signed` call)
- Vault token accounts themselves are set up via `naclac_client`'s own
  client-side helpers (`create_token_account`) rather than a second
  on-chain `init` instruction — matching how `examples/escrow` does it
  (`vault_token_account` has no `init` constraint at all; the account is
  pre-created off-chain before the instruction that checks it runs)

**`token::program` and `mint::freeze_authority` — ✅ built and passing
(4/4, all 3 modes: `token`, `token_borsh`, `token_zc`).**

- `check_vault_program` (`programs/*/src/instructions/check_vault_program.rs`,
  all 3 modes) — `token::program = token_program` on an existing, bare
  `AccountInfo` `vault` field, checked in `generate_relational_checks`
  (`security.rs`, ~lines 762-772, post account-load):
  `Owner::program_owner(&vault) == ToAddress::address(&token_program)`.
  Positive case: a real SPL token account (created via
  `create_mint`/`naclac_client::create_token_account`, genuinely owned by
  the real Token program) is accepted against a `Program<Token>` field
  pointed at the real Token program. Negative case: the `mint_authority` PDA
  (owned by our own program, not the Token program) is passed as `vault`
  instead, expecting `NaclacError::ProgramIdMismatch` — confirmed distinct
  from `token::mint`'s `ConstraintAccountIsNone` and `token::authority`'s
  `ConstraintAddress` by reading the codegen directly. Avoids needing a
  second, separately-deployed token program (e.g. Token-2022) in `litesvm`
  for the negative case — any non-Token-owned account reaches the relational
  check the same way, since `Account<TokenAccount>`'s own `try_from`
  (`naclac-token/src/token.rs`, `discriminator_len() == 0`) and bare
  `AccountInfo` perform no owner check of their own at load time (confirmed
  by reading `naclac-core/src/wrappers/account.rs` and `account_loader.rs`
  directly, not assumed).
- `create_mint_with_freeze` (`programs/*/src/instructions/create_mint_with_freeze.rs`,
  all 3 modes) — mirrors `create_mint.rs` exactly but additionally sets
  `mint::freeze_authority = mint_authority`, exercising the `init`+CPI
  freeze-authority branch in `init_cpi.rs` (~lines 185-211 non-pinocchio,
  ~608-621 pinocchio) that `create_mint.rs` alone never touched. Reuses the
  same `SEED_MINT` prefix as `create_mint`; PDA uniqueness still comes from
  the dynamic `id` seed component, same as `create_mint`. No manual
  bump-write-back workaround needed (unlike `init_mint_authority`'s bare-bump
  gotcha noted above) since the `mint` field is a bare `AccountInfo` with an
  *explicit* `bump = mint_bump`, not a `#[component]`.
- `check_mint_freeze_authority` (`programs/*/src/instructions/check_mint_freeze_authority.rs`,
  all 3 modes) — `mint::freeze_authority = freeze_authority` on an existing,
  non-`init` `Account<Mint>` field (`security.rs`, ~lines 611-638, only
  reachable when `field.init_config.is_none()`): reads the raw SPL `Mint`
  layout's `COption` discriminant at bytes `[46..50]` and, if present, the
  freeze authority pubkey at `[50..82]`. Three cases in one test: (1) the
  matching authority (the `mint_authority` PDA used at creation time) is
  accepted; (2) a mismatched authority (a fresh, unrelated keypair) is
  rejected with `NaclacError::ConstraintAddress`; (3) a mint created via the
  *original* `create_mint` (never had a freeze authority set at all — the
  `COption` discriminant is `[0,0,0,0]`) is rejected with
  `NaclacError::Unauthorized` — deliberately distinct error variants,
  confirmed by reading the codegen directly rather than assumed.

**Real bug found and fixed while verifying this**: `check_mint_freeze_authority`
was originally written with `mint` declared *before* `freeze_authority` in
the struct. Field constraints reference sibling fields by bare local
variable name, and fields load sequentially in declaration order
(`accounts.rs`'s `field_loaders`, spliced in declared order) — so a field a
constraint references must be declared *before* the field using it (the
same convention `check_vault_constraints.rs` already follows: `mint`/
`mint_authority` precede `vault`). Reordered `freeze_authority` before
`mint` in all 3 variants; not a framework bug, a test-authoring mistake
caught by the real build.

**`AssociatedToken` derivation/creation — ✅ built and passing (5/5, all 3
modes: `token`, `token_borsh`, `token_zc`).** This was previously deferred
as "too risky to guess at blind" — that turned out to be overly cautious;
`litesvm` already bundles the real Associated Token Account program (see
below).

Initially built as a manual `AssociatedTokenCpi::create`/`CreateAtaAccounts`
CPI call in the instruction body (mirroring `mint_to_vault.rs`/
`transfer_tokens.rs`'s convention for manual CPIs). While reviewing this,
the asymmetry with `create_mint`'s fully declarative `mint::decimals`/
`mint::authority`/`mint::freeze_authority` sugar was flagged — ATA creation
had real underlying CPI machinery (`naclac_lang::associated_token::create`)
but nobody had ever wired equivalent `associated_token::mint`/
`associated_token::authority` constraint keys into the macro parser, so
users were stuck hand-rolling the CPI. **Built that declarative sugar
instead of leaving the manual version as the final state**:

- `naclac-macros/src/instruction/parser.rs` — added
  `associated_token_mint`/`associated_token_authority` to `ParsedField`,
  recognized `associated_token::mint`/`associated_token::authority` as a
  reserved 2-segment key (same pattern as `mint::X`/`token::X`).
- `naclac-macros/src/instruction/init_cpi.rs` — new codegen branch,
  inserted right after `mint::decimals`. Unlike `mint::decimals` (which
  hand-rolls raw CPI instruction bytes per backend, since there's no shared
  CreateAccount+InitializeMint helper), this reuses
  `naclac_lang::associated_token::create` directly — that function already
  branches on `#[cfg(feature = "pinocchio")]` internally (confirmed by
  reading `naclac-token/src/associated_token.rs`), so the generated code
  needs no cfg branches of its own, the same "already unified at the call
  site" pattern established for `system_program::transfer` in `realloc.rs`.
- `create_ata.rs` (all 3 variants) rewritten to use
  `#[account(init, payer = payer, associated_token::mint = mint,
  associated_token::authority = owner)]` on the `associated_token` field —
  the instruction body is now empty, exactly mirroring `create_mint`'s.

Accounts: `payer: Signer (mut)`, `owner: AccountInfo` (the wallet that will
own the new ATA — a fresh `Keypair`, deliberately distinct from the payer,
see bug #3 below), `mint: Account<Mint>` (from the existing `create_mint`
instruction), `associated_token: AccountInfo` (`init`-constrained — doesn't
exist on-chain yet, so a bare `AccountInfo`, not `Account<TokenAccount>`,
mirroring `create_mint.rs`'s own `mint: AccountInfo` reasoning),
`system_program: Program<System>`, `token_program: Program<Token>`,
`associated_token_program: Program<AssociatedToken>`. The test
(`create_ata_creates_a_real_associated_token_account`, all 3 `tests/*.rs`
files) creates a mint via `create_mint`, derives the real ATA address
off-chain via `Address::find_program_address(&[owner, token_program, mint],
ASSOCIATED_TOKEN_PROGRAM_ID)` (mirroring `naclac_client::create_ata_with_program`'s
own derivation), calls `create_ata`, then fetches the resulting account's
raw bytes and confirms it deserializes as a valid SPL token account: owned
by the real Token program, with `mint` (bytes `[0..32]`) and `owner` (bytes
`[32..64]`) both set correctly — proving the CPI genuinely created a real,
correctly-owned token account, not just that the instruction didn't error.
Passed on the first build with the declarative version, no follow-up fix
needed. No extra `.add_program()` call was needed for the Associated Token
Program itself — confirmed directly from the `litesvm` crate source
(`litesvm-0.12.0/src/programs/mod.rs`'s `load_default_programs`) that
`LiteSVM::new()` already preloads the real SPL Token program, Token-2022,
and the real Associated Token Account program by default.

**Real framework bug #1, found and fixed while building this**:
`naclac-token/src/lib.rs`'s `prelude` module re-exported
`associated_token::Create as CreateAta` (the low-level, internal accounts
struct) but never re-exported the `AssociatedTokenCpi` trait or the
public-facing `CreateAtaAccounts` struct. Since `.create(...)` is a trait
method, `use naclac_lang::prelude::*` alone would never bring
`AssociatedTokenCpi` into scope — any instruction body calling
`ctx.accounts.associated_token_program.create(...)` would fail to compile
with "no method named `create` found," regardless of how correctly the rest
of the call site was written. Fixed by adding
`pub use crate::associated_token::{AssociatedTokenCpi, CreateAtaAccounts};`
alongside the existing `CreateAta` re-export in `naclac-token/src/lib.rs`'s
`prelude` module.

**Real bug #2, caught by the actual build**: the `#[derive(Accounts)]`
struct for this instruction was originally also named `CreateAta`, which
collides with `naclac_lang::prelude`'s own `associated_token::Create as
CreateAta` re-export — both are visible via `lib.rs`'s two glob imports
(`naclac_lang::prelude::*` and `instructions::*`), producing an ambiguous-name
error at the `Context<CreateAta>` call site. Renamed the Accounts struct to
`CreateAssociatedTokenAccount` in all 3 variants.

**Real bug #3 (test-authoring, not framework), caught at runtime by
naclac's own security check**: the test originally set `owner =
provider.payer.address()`, reusing the payer's own pubkey as the ATA's
owner. Solana collapses a pubkey repeated across multiple account slots in
one transaction into a single writable meta if *any* occurrence requests
`mut` — so `owner` silently became writable too, aliasing the same slot as
`payer` (`#[account(mut)]`), which `NaclacError::ConstraintDuplicateMutableAccount`
correctly rejected. This is not something that could ever be a compile-time
check: which accounts share a pubkey depends on what's actually passed into
a specific invocation, not on the struct's shape. Fixed by using a fresh,
distinct `Keypair::new()` for `owner` instead.

Deferred to a follow-up (documented, not silently dropped):
- Token-2022 variant specifically — the CPI helpers already branch on
  `program.address() == spl_token_2022::ID` internally (confirmed in
  `naclac-token/src/token.rs`), so this is a matter of parameterizing the
  existing instructions over `token_program`, not new code — left for the
  same follow-up pass as the ATA case. (Note: `check_vault_program`'s
  negative case above deliberately doesn't need a real, separately-deployed
  Token-2022 program for its own purposes — see above.)

**Note:** runs against real SPL Token program logic via `litesvm` (already a
workspace dependency per root `Cargo.toml`), not a mocked CPI — a mocked
token CPI would defeat the point of testing token constraint enforcement.

---

## 7. `tests/events/` — event emission across all three event mechanisms ✅ DONE

**✅ Built and passing (6/6, all 3 modes).** `tests/events/` exists (3 modes: `events`,
`events_borsh`, `events_zc`) — a single `Counter` component + `init_counter`/
`increment_counter` instructions, mirroring `examples/counter` exactly.
`increment_counter` emits `CounterIncremented { new_count }` via `emit!` on
every call; the test calls it twice and decodes both transactions'
real log output (`tx_meta.parse_events_zero_copy::<CounterIncremented>()` in
pinocchio/zc, `.parse_events_borsh()` in borsh — the only difference between
the 3 mode variants, confirmed from `naclac-client/src/provider.rs`'s two
separate parse methods), asserting `new_count` is `1` then `2` — proving the
payload reflects real, current on-chain state each time, not a stale or
hand-constructed value. A second instruction, `touch_counter_explicit_bump`,
was added specifically to verify — not assume — that explicit
`bump = counter.bump` (a self-reference on an *existing*, non-`init`
account) works: no other test in this repo exercises that exact form, only
bare `bump`'s auto-path. Reasoned from `security.rs` to be equivalent (bare
bump's auto-path literally synthesizes `#field_ident.bump` as the same
expression), but reasoning about framework code isn't the same as verifying
it, and this whole `tests/` effort exists to close exactly that gap. This
directly covers both bullets below: the
round-trip itself, and (since the parse methods compute their own expected
discriminator via `sha256("event:CounterIncremented")[..8]` independently of
whatever the on-chain `emit!` call produced) a genuine discriminator-match
proof, not an assumed one.

**Two real bugs found and fixed, both in the test program, not the
framework** — and one genuine framework nuance confirmed safe by tracing it
rather than guessing:
- Calling the same zero-arg instruction on the same account twice in a row
  produced byte-identical transactions, which litesvm correctly rejects as
  a replay (`AlreadyProcessed`) — not a naclac bug. Fixed with
  `litesvm::LiteSVM::expire_blockhash()`, reachable via `NaclacProvider`'s
  public `backend: ClientBackend::LiteSVM(Arc<Mutex<LiteSVM>>)` field.
  Worth remembering for any future case that calls the same no-args
  instruction on the same accounts more than once per test.
- `init_counter` never wrote `counter.bump` — same class of bug as
  `init_mint_authority` in `tests/token/` (bare `bump`'s auto-write-back is
  zero-copy-only; solana-borsh mode needs
  `ctx.accounts.counter.bump = ctx.bumps.counter;` written explicitly).
  This is now the *second* case to hit this exact gotcha — worth treating
  as a standing rule for any future case using bare `bump` on an `init`
  field meant to run in solana-borsh mode.
- Tracing *why* the explicit-bump test caught the bug that bare bump's own
  test didn't (rather than just patching and moving on) surfaced a real,
  confirmed-safe asymmetry: for a **pure-literal, precomputed-PDA seed**
  on an *existing* (non-`init`) account, bare `bump`'s auto-path skips
  runtime bump verification entirely (`security.rs`'s precomputed branch,
  `bump_name == "__naclac_auto_bump"` short-circuits past the verification
  block) — it relies solely on the address check, which is still
  cryptographically sufficient alone. Explicit `bump = <expr>` always
  performs the extra comparison. Not a gap, just an asymmetry worth knowing
  about if a future case's assertions depend on bump *mismatches* being
  caught on an existing, non-`init`, literal-seeded account with bare bump.

**Modes:** all three (`naclac-core/src/event/{borsh,zero_copy,pinocchio}.rs`
are three genuinely different implementations, not one implementation with
cfg branches — confirmed by reading all three: borsh concatenates
discriminator+serialized-payload into one `sol_log_data` slice, zero-copy/
pinocchio instead pass discriminator and raw `bytemuck` bytes as two
separate slices in the same call).

- Emit an event, decode it back from the transaction's log output, confirm
  field values round-trip correctly in each of the three backends
- Confirm the discriminator prefix on the emitted event matches what the
  client-gen'd decoder expects (this is the join point with `tests/client-gen/`
  below — an event encoding bug would show up as silent decode failure on
  the generated client, not a framework-side error)

---

## 8. `tests/cpi/` — cross-program invocation ✅ DONE (core subset)

**✅ Core subset built and passing (1/1).** `tests/cpi/` exists as **two** real,
separately-deployed programs (`cpi_callee`, `cpi_caller`) — genuine
cross-program CPI needs two actual programs, not one self-CPI'ing one.
`cpi_caller` depends on `cpi_callee`'s auto-generated client SDK
(`cpi-callee-client`, `features = ["cpi"]`) exactly like
`examples/launchpad`'s `token_creator` depends on `amm-client` — this is
the real mechanism a naclac user relies on for cross-program CPI, not a
hand-rolled `invoke` call (confirmed via `naclac-client-gen/src/rust/instructions/cpi.rs`:
`{ix_camel}CpiAccounts`/`{ix_camel}Cpi` trait with `{ix_snake}`/`{ix_snake}_signed`
methods, generated per-instruction, gated on the `cpi` feature).

- `call_system_transfer` — real CPI to the System Program
  (`SystemTransferAccounts`)
- `call_setup_counter` — real cross-program CPI (unsigned) to `cpi_callee`'s
  `init_counter`, via the generated `InitCounterCpi` trait
- `call_authorized_increment` — real cross-program CPI **signed with PDA
  seeds**: `cpi_callee`'s `authorized_increment` requires its `authority`
  account to sign, and that authority is recorded (at setup time) as
  `cpi_caller`'s own PDA — a PDA can't sign like a keypair, so this
  exercises the real `invoke_signed`-with-PDA-seeds path end to end, not a
  keypair standing in for one

**Two of my own mistakes caught and fixed before this ever reached the
user, same class of error each time:** `callee_program: Address::default()`
— unlike `system_program` (whose real address genuinely is all-zero bytes,
confirmed repeatedly this session), a custom `Program<CpiCallee>` has no
well-known fixed address the client can auto-resolve; it needs the real,
dynamically-deployed `CALLEE_PROGRAM_ID` explicitly. Worth remembering: the
`Address::default()` shortcut is valid *only* for `System`/`Token`-style
well-known programs, never for a custom cross-program dependency.

**Known first-build bootstrap order issue:** `cpi_caller` depends on
`cpi-callee-client`, which only contains the real generated CPI
trait/accounts *after* `cpi_callee` itself has been built and had
`naclac generate` run on it at least once. `Naclac.toml` lists `cpi_callee`
first specifically so `naclac build`'s per-program loop reaches it first —
but if the very first-ever build still fails on `cpi_caller` against a stub
client, just re-run `naclac build` a second time (the real client will
already be on disk from the first pass' `cpi_callee` step by then). This
isn't a bug — the same bootstrap shape exists in `examples/launchpad`
itself, just invisible there because its clients were already committed to
git from a prior successful build. Confirmed in practice: the first real
run hit exactly this — `naclac build` failed on `cpi_callee` itself with a
transient `Permission denied` writing `.rmeta` (the same NTFS-via-9p WSL
flakiness noted at the top of this doc, this time surfacing during
`naclac build` rather than `cargo test`), which meant `cpi_callee`'s client
never got generated for real that pass, leaving `cpi_caller` building
against the stub. A second `naclac build` run resolved it — the SBF compile
succeeded on retry and the real client generated correctly.

**One real bug in the test itself, same class as before:** the generated
client's `Counter.authority` field is `cpi_callee_client::sdk_core::Address`
(naclac-core's on-chain type) while `get_caller_authority_pda` returns
`solana_address::Address` (the off-chain/client type) — two genuinely
different types with the same name, the exact distinction noted earlier
this session. Fixed with `.into()` at the comparison site. Worth
remembering for any future cross-program-CPI test: comparing an on-chain
component field against an off-chain-derived PDA address will hit this
every time.

**Deferred to a follow-up, not silently dropped:** a dedicated test that
actually *triggers* `MAX_CPI_SIGNERS`/`MAX_CPI_SEEDS_PER_SIGNER` and
confirms the new `TooManyCpiSigners`/`TooManyCpiSeeds` errors fire (see the
real bug found and fixed in `naclac-core/src/cpi.rs` and
`naclac-token/src/token.rs` — both used to silently truncate past these
limits instead of erroring). The high-level generated CPI trait methods
used here only ever pass one PDA signer, so forcing an overflow needs a
lower-level raw `pinocchio::instruction::InstructionView` construction
whose exact API I hadn't verified — rather than guess at an unfamiliar
low-level API blind, this is left as a follow-up with a known starting
point (`naclac_lang::prelude::invoke_signed_pinocchio`/`invoke_signed_pinocchio_handles`,
both already fixed) instead of a fragile, unverified test.

**Modes:** pinocchio only for this first pass (two-program CPI wiring
across 3 modes each — 6 program variants, cross-referencing each other —
was judged more complexity than this first pass warranted; borsh/zc parity
is a natural follow-up once this is confirmed working).

---

## 9. `tests/stack-safety/` — the `Box<Account<T>>` boxing mechanism ✅ DONE (compile-time asserts)

**Modes:** solana-borsh only (zero-copy is explicitly out of scope for this
mechanism per the stack-overflow fix plan — `Account<T>`'s zero-copy branch
doesn't embed the full struct inline, so it isn't subject to the same
frame-size math).

`tests/stack-safety/` is a real, deployable single-mode program
(`Naclac.toml`, `programs/stack_safety/`, built/tested via `naclac build` +
`scripts/test-all.sh stack-safety`, same shape as every other test dir),
plus a nested `compile-fail/` `trybuild` crate for the two cases that can
only be proven by a failed compile — a program-level negative case can't
be "run" the normal way, since an unboxed oversized field fails the whole
crate's build. The `trybuild` crate has its own `Cargo.toml`/`[workspace]`
with the `borsh` feature enabled (the asserts only exist when
`is_zero_copy` is false), separate from the shared `tests/compile-fail/`
crate, which has no `borsh` feature and is permanently in zero-copy mode —
it could never reach this code path.

**`compile-fail/` (2 fail + 2 pass fixtures):**
- **A struct with one field over the per-field budget** — `tests/fail/01_per_field_oversized.rs`
  (`Account<BigData>`, a single `[u8; 300]` field, guaranteed over the
  300-byte cap regardless of `AccountInfo`'s own overhead) confirms the
  `const _: () = assert!(...)` in `accounts.rs:332` fires; `tests/pass/01_per_field_boxed_clears_it.rs`
  is the identical fixture boxed, confirming `Box<Account<T>>` clears it.
- **Several fields each under budget individually but over in aggregate** —
  `tests/fail/02_aggregate_oversized.rs` (nine `Account<MidData>` fields,
  each `[u8; 200]`, individually under 300 but summing well past 1700)
  confirms the aggregate assert (`accounts.rs:352`) fires on a shape the
  per-field check alone can't catch; `tests/pass/02_aggregate_partially_boxed_clears_it.rs`
  boxes 5 of the 9 fields, confirming boxing doesn't have to be
  all-or-nothing — the remaining 4 unboxed fields fit under budget on
  their own.

**`programs/stack_safety/` (real, deployed, litesvm-tested):**
- **A struct whose fields sum under both budgets, no boxing required** —
  `SmallData` (`bump: u8, value: u64`), used unboxed by `init_small`/`touch_small`.
- **`Box<Account<T>>` is load-bearing, not decorative** — `BigData`'s
  300-byte payload puts it over the per-field budget for real; every field
  of this type in the program is `Box<Account<BigData>>`, or the crate
  doesn't compile at all (confirmed by the `compile-fail/` fixtures above
  being the unboxed version of this exact shape). `init_big` is also the
  first place in the repo `init` targets a `Box<Account<T>>` field
  specifically (`init_cpi.rs`'s `extract_inner_type` already had explicit
  `Box<...>`-unwrapping logic for this — confirmed working, not just
  present in the code).
- **Transparency, two different blanket-impl paths**: `touch_big` exercises
  `Deref`/`DerefMut` (mutation) and `ToAddress` (`.address()`) through the
  Box; `close_big` exercises `ToAccountInfo` (the lamport-drain/reassign
  path `close_account.rs` uses) through the Box — a different code path
  than mutation, not redundant with it. (`tests/dup-mut/`'s
  `boxed_account_field_loads_and_mutates_like_an_unboxed_one` already
  covered the mutation/address side on a pre-existing component; `close_big`
  here is the new ground, not a duplicate.)
- **Deferred, not attempted here:** actually invoking `naclac build` on an
  intentionally-oversized struct and asserting the CLI's own process exit
  code/output (`build.rs`'s PTY output-scanning for "overflows the maximum
  allowed frame space", independent of the macro-level assert — the one
  case that heuristic can't see, since it only inspects Accounts-struct
  field types, not arbitrary function-local stack variables). This is
  process/CLI-behavior testing, the same shape section 13 (`tests/cli/`)
  already carves out as needing its own harness rather than `cargo test` —
  left there rather than building a one-off verification for it here.

---

## 10. `tests/instruction-attrs/` — `close_account`, `realloc`, `init_cpi` codegen paths ✅ DONE (via other cases)

**Modes confirmed:** solana-borsh and pinocchio genuinely is a 2-way split
for `close_account.rs`/`realloc.rs` — both only ever branch on
`#[cfg(feature = "pinocchio")]` vs. not, with no separate zero-copy-specific
code path in either file (confirmed by reading both directly, not assumed).

All three bullets below ended up covered by existing cases rather than
needing a fresh one:
- **`close`** — `tests/accounts-constraints/`'s `close_vault` (lamports
  drained to destination, account unusable afterward)
- **`init` via CPI to the system program (`init_cpi.rs`)** —
  `tests/token/`'s `create_mint` (`init` + `mint::decimals`/`mint::authority`,
  the CPI-based mint-creation path)
- **`realloc` growing and shrinking, with `zero`** — **✅ `tests/realloc/`
  built and passing (4/4: `growing_resizes_funds_and_zeroes_new_bytes` +
  `shrinking_resizes_and_refunds_excess_rent`, on both `realloc` (pinocchio)
  and `realloc_borsh` (solana-borsh))**. This case turned out to be by far
  the buggiest single feature found this whole session — `realloc` had
  never been used by any example in the repo, and was effectively
  non-functional end-to-end before this. Real bugs found and fixed, in the
  order they surfaced:
  - **`realloc`/`realloc::payer`/`realloc::zero` couldn't even parse**:
    missing from `is_reserved` in `naclac-macros/src/instruction/parser.rs`,
    so `realloc::payer = payer` fell into generic relation-field parsing and
    errored with "Expected relation field name".
  - **`fully_parse_realloc` searched for the wrong key names**: copy-pasted
    from `fully_parse_init`, it searched for `"space = "`/`"payer = "`
    instead of `"realloc="`/`"realloc::payer="` — meaning `field.realloc`
    could never have been populated at all, independent of the parsing fix
    above.
  - **A whitespace-stripping fix for the above then broke expressions**:
    stripping all whitespace to make key-matching reliable corrupted
    multi-token expressions like `new_space as usize` into the single
    bogus identifier `new_spaceasusize`. Fixed by normalizing only the
    `::`/`=` separators instead of stripping all whitespace.
  - **`AccountInfo::resize()` didn't exist**: naclac's own lifetime-erased
    `AccountInfo` wrapper (`naclac-core/src/prelude.rs`, one definition per
    backend) never had a `resize` method written at all. Added one to each:
    the solana-backend wrapper forwards to the real
    `solana_program::account_info::AccountInfo::resize` via the existing
    `to_lifetime()` cast; the pinocchio wrapper uses pinocchio's `Resize`
    trait (requiring the `account-resize` feature to be enabled on the
    `pinocchio` dependency in the root `Cargo.toml`, which it wasn't).
  - **Pinocchio growth was never funded at all**: the original code just
    resized on the pinocchio path with no lamport top-up whatsoever (silent
    under-funding on growth). Rewrote `generate_realloc_logic` in
    `naclac-macros/src/instruction/realloc.rs` to use the framework's
    existing dual-backend `naclac_lang::prelude::system_program::transfer`
    helper (already used elsewhere for CPI transfers on both backends)
    instead of hand-rolled invoke code, so growth funding now genuinely
    works under pinocchio too.
  - **Shrink refund added**: previously the account just stayed
    over-funded forever after shrinking; now the excess rent is refunded
    directly back to the payer (mirrors `close_account.rs`'s exact
    direct-lamport-debit/credit pattern — no CPI needed since the target
    account is already owned by the program).
  - **`realloc::zero` corrected, not a safety bug after all**: tracing the
    underlying `resize()` primitives directly (`solana-account-info`'s
    `sol_memset(&mut data[old_len..], 0, ...)` and pinocchio's
    `AccountView::resize`'s own `write_bytes(..., 0, ...)`) showed both
    backends *already* unconditionally zero newly-grown bytes themselves —
    `zero = true` was always redundant, not silently unsafe. The dead
    `_zero_flag` variable is now a documented, deliberate no-op rather than
    unexplained dead code, and the test directly observes the zeroing
    happens (not just trusts the code reading).
  - **`realloc = <expr>` referencing an `#[instruction(...)]` arg couldn't
    resolve**: e.g. `realloc = new_space as usize` — `new_space` only
    exists as a local inside the generated `load_and_validate`, but the
    realloc logic runs in `teardown`, a separate function with no access to
    it. Fixed by threading the pre-evaluated space through the
    auto-generated `Bumps` companion struct (`accounts.rs`), the same
    channel PDA bump seeds already use to cross that exact boundary — added
    a `__realloc_space_<field>: usize` field, computed once in
    `load_and_validate` where the instruction-arg local is in scope, read
    back in `teardown` via `bumps.__realloc_space_<field>`.

---

## 11. `tests/error-codes/` — `#[error_code]` and error propagation

**✅ Built and passing (3/3).** `tests/error-codes/` exists (one mode,
`error_codes`, solana-borsh — macro-syntax-level, not backend-dependent,
per this section's own guidance). `TestError`
(`ZeroAmount`/`Unauthorized`/`TooLarge`) confirmed from
`naclac-macros/src/error_code.rs` directly: no explicit discriminants
means the 6000 offset lands them at exactly 6000/6001/6002
(`e as u32 + 6000`). `check_amount`/`check_authority` trigger each one via
`require!`, positive + negative each.

**Real bug found and fixed in `naclac-client-gen` while building this
(not the framework itself, but the CLI's client generator):**
`naclac-client-gen/src/rust/lib_generator.rs` unconditionally emitted
`pub mod components;` regardless of whether the program had any
`#[component]`s at all — `naclac-client-gen/src/rust/mod.rs` only creates
the `src/components/` directory when `idl.accounts` is non-empty, so a
pure-logic program (no components, like this one) got a generated client
that failed to compile (`E0583: file not found for module`). Fixed by
gating both the `mod`/`use` declarations on the same `idl.accounts`/
`idl.instructions` emptiness checks already used for directory creation.
This required reinstalling the `naclac` CLI (`cargo install --path cli`)
since `naclac-client-gen` compiles into the CLI binary itself, not the
user's program.

**One of my own test-design mistakes, caught and fixed:** originally tried
to prove the `NaclacError`/custom-error codespace separation using a
missing `Signer` signature (expecting `NaclacError::ConstraintSigner`,
3002). That never reaches the chain at all — Solana's own client-side
transaction-signing rules refuse to even construct a transaction missing a
signature for a declared signer (`NotEnoughSigners`), so the on-chain
constraint check never runs. Replaced with `check_fixed_address`
(`address = SYSTEM_PROGRAM_ID`), which *is* a validly-signable transaction
that fails on-chain — landing at `3000 + 0*100 + ConstraintAddress(3) =
3003`, genuinely proving the two codespaces don't collide in practice.
The missing-signature case was kept as its own, differently-shaped test
(`missing_signer_signature_is_rejected_before_reaching_the_chain`) since
it's a real, worth-knowing property in its own right — just not proof of
codespace separation.

**Modes:** one mode is sufficient (this is macro-syntax-level, not
backend-dependent) — pick solana-borsh since it's cheapest to iterate on.

- Custom error enum with `#[error_code]`, confirm each variant's numeric
  code + message surfaces correctly in a failed transaction's logs
- Confirm `NaclacError` framework errors (from `naclac-core/src/error.rs`)
  and program-defined errors don't collide in code-space

---

## 12. `tests/idl-and-clientgen/` — the IDL → client generation pipeline

**Modes:** one program per mode (idl-build feature is orthogonal to
solana-borsh/zerocopy/pinocchio, but the generated IDL's type mappings
*do* differ by mode — the consistency plan's Part 3 verification step
already flags "zero-copy IDL uses different wrapper types than Borsh IDL"
as something to spot-check).

- Generate IDL (`naclac-idl`) for one program per mode, confirm `is_zero_copy`
  (or equivalent field) is correct per mode
- Generate the Rust client (`naclac-client-gen/src/rust/*`) and confirm it
  compiles standalone against the IDL
- Generate the TypeScript client, both `kit.rs` (new `@solana/kit`-style) and
  `legacy.rs` (`@solana/web3.js`-style) output variants — confirm both
  actually run against a local validator/litesvm, not just typecheck
- Round-trip: encode an instruction with the generated client, submit it,
  confirm the on-chain program decodes it correctly (this is the real
  end-to-end check — typechecking the generated code alone doesn't catch a
  wire-format mismatch)

**Real bug found and fixed while building `tests/pda-seeds/`** (not
hypothetical — worth a dedicated regression case here): `get_child_pda` in
the generated Rust/TS clients was silently wrong whenever a PDA seed
referenced a *field* of another account (e.g. `registry.bump`, a `u8`)
rather than that account's own address. All three generators
(`naclac-client-gen/src/rust/lib_generator.rs`, `.../typescript/kit.rs`,
`.../typescript/legacy.rs`) treated any `IdlSeed::Account { path }` as a
32-byte `Address` regardless of whether `path` was a plain account name or
a dotted field-access path.

**Fixed properly, not just contained.** Threaded the backing component
type through the whole pipeline, mechanically — no guessing at any step:
`naclac-syn/instruction.rs` now extracts the inner generic type from
`Account<T>`/`InterfaceAccount<T>` fields (the same technique already used
for `Program<T>`, just previously not applied to `Account<T>`), `pda.rs`
resolves a dotted seed's real field type by looking it up in that
component's own already-parsed struct definition, and the resolved type
(`NaclacSeed`/`IdlSeed::Account`'s new `field_type: Option<String>`) flows
through `naclac-idl`'s JSON schema into all three generators, which now
emit a correctly-typed, correctly-encoded parameter (`registry_bump: u8` in
Rust, `registry_bump: number` in TS, both converting via the same
byte-conversion rules already used for plain instruction-arg seeds) instead
of guessing `Address`. Verified end-to-end against `tests/pda-seeds`'s real
`get_child_pda` output in all three generators.

The skip-with-explanatory-comment behavior is now only the *residual*
fallback — it still fires, correctly, when a referenced field's type is
something naclac-syn can't resolve to a plain primitive (a nested/defined
type, not `u8`/`u16`/`publicKey`/etc.), since there's still no safe way to
generate byte-conversion code for an arbitrary nested type.

**✅ DONE.** Both paths are covered in `tests/pda-seeds/` (both modes):
- `generated_pda_helpers_agree_with_on_chain_program` — primitive
  field-type seed (`registry.bump: u8`, via `init_child`/`get_child_pda`):
  asserts the generated helper is present *and* produces the same PDA the
  on-chain program actually accepts.
- `non_primitive_field_seed_pda_helper_is_correctly_skipped` — non-primitive
  field-type seed (`registry.label: [u8; 4]`, a new field + `TaggedChild`
  component/`init_tagged_child` instruction added specifically for this):
  asserts the on-chain program still accepts the hand-derived PDA, while
  the generated `get_tagged_child_pda` helper is correctly absent (checked
  by reading the generated `clients/rust/*/src/lib.rs` source directly) with
  the explanatory "intentionally not generated" comment present instead of a
  guessed implementation.

Both run together via `cd tests/pda-seeds && naclac build && cd ../.. &&
./scripts/test-all.sh pda-seeds` — 3/3 tests passing in each of `pda_seeds`
and `pda_seeds_zc`.

---

## 13. `tests/cli/` — `naclac` command surface (not `cargo test`-shaped; needs its own harness)

These test the CLI binary's *behavior*, not naclac-core/-macros. Likely
needs a shell/integration-test harness (e.g. `assert_cmd` crate) rather than
fitting the `tests/<case>/Cargo.toml` workspace pattern used above — flagging
that distinction rather than prescribing the wrong shape.

- `naclac init` for each template/mode combination — confirm generated
  project actually builds (this is exactly what Anchor's `template-tests.yaml`
  matrix does, and it's the one workflow-file idea from that survey worth
  eventually adopting once this exists)
- `naclac build` — success path, and the stack-overflow-detection failure
  path from section 7 above
- `naclac test`, `naclac deploy` (against a local validator), `naclac idl`,
  `naclac generate`
- `naclac add` — adding a dependency/component to an existing project
- `naclac account`, `naclac logs`, `naclac doctor`, `naclac profile` — these
  are read/introspection commands; lower priority than the write-path
  commands above but worth at least a smoke test each

---

## 14. `tests/nacvm/` — version manager

Separate concern from the framework itself, lower priority than everything
above. `install`, `use`, `uninstall`, `list` against a fake/mocked release
index rather than hitting real GitHub releases in CI.

---

## 15. `tests/optional-accounts/` — `Option<T>` account fields (sentinel scheme) ✅ DONE

**✅ Built and passing (12/12 `optional_accounts`, 14/14 `optional_accounts_borsh`,
14/14 `optional_accounts_zc` — 40/40 total).** Covers the sentinel design in
[naclac-macros/docs/optional-accounts-plan.md](../naclac-macros/docs/optional-accounts-plan.md):
an `Option<T>` field always occupies its declared slot; absence is signaled by
the caller passing the program's own address in that slot, checked against
`program_id` in the generated `load_and_validate`.

Per backend: `touch_optional`/`touch_two_optional` (present, absent,
wrong-account-when-present rejected, both present, the `MUT_MASK`-regression
both-absent case, mixed presence) and `init_optional_thing`/
`close_optional_thing`/`realloc_optional_thing` (present/absent for each).
`optional_accounts_borsh`/`optional_accounts_zc` additionally cover
`touch_boxed_optional` (`Option<Box<Account<T>>>`, present/absent).

Real bugs found and fixed while building this (framework code, not test
code — full detail in the plan doc):
- `MUT_MASK` didn't exclude `Option<T>` `mut` fields — two omitted optional
  `mut` accounts (both holding the identical sentinel address) would have
  falsely tripped the duplicate-mutable-account guard.
- The "remaining accounts duplicate check" in `accounts.rs` called
  `ToAddress::address()` unconditionally on every `mut` field — a runtime
  `if` guard isn't enough to avoid the type error, since Rust still
  type-checks a branch whose condition is a literal `false`; optional
  fields had to be excluded from the token generation itself.
- `naclac-syn`'s IDL generator used to detect "optional" via a bare keyword
  in `#[account(...)]` attributes that `naclac-macros`' real parser never
  recognized — completely disconnected from the actual runtime mechanism.
  Replaced with real `Option<T>` structural detection.
- `naclac-client-gen` was *omitting* absent optional accounts from generated
  transactions instead of sentinel-filling them — would have silently
  misindexed every account declared after an absent optional one.
- `naclac-core/src/prelude.rs`'s `no-std`/`pinocchio`/`alloc` routing had a
  latent duplicate-import bug (only surfaced under `--all-features`) and a
  real gap where standalone `no-std` (without `pinocchio`) never linked
  `extern crate alloc` at all.
- `init`/`close`/`realloc`/seeded-`bump` on `Option<T>` fields were
  initially rejected at parse time; all four are now supported (`init` and
  `close`/`realloc` needed different fixes — `init` already ran inside the
  sentinel gate before `self` existed, `close`/`realloc` run in `teardown()`
  after `self` exists and needed real per-field guards; seeded-`bump` needed
  an `Option<u8>` bump that survives the sentinel branch's scope).

**Modes:** all three. `Option<Box<T>>` works with zero macro changes (`Box<T>`
was already unwrapped before the `Option<>` check runs); `Box<Option<T>>`
(the reverse nesting) remains unsupported — see the plan doc's "Deferred"
section.

---

## Priority order (highest leverage first)

1. **`tests/discriminator/`** ✅ DONE — security property, cheap to write,
   three modes, 6/6 passing.
2. **`tests/pda-seeds/`** ✅ DONE — proven bug-prone area, previously-confirmed
   zero-coverage gap (auto-bump) now covered, 2/2 passing.
3. **`tests/compile-fail/`** ✅ DONE (14/15 sites; 15th needs a different
   testing shape, see section 5) — cheap per-site (`trybuild`, no
   litesvm/program compile needed). 4 legacy-argument checks were removed
   from the framework outright rather than tested (see section 5).
4. **`tests/dup-mut/`** ✅ DONE — 20/20 passing across all 3 modes. Two real
   client-gen bugs found and fixed (`ZcString` IDL mapping, zero-copy
   dynamic-arg wire encoding).
5. **`tests/accounts-constraints/`** ✅ DONE — 42/42 passing across all 3
   modes. Found a real gap in `NaclacProvider` (no way to fund an account
   below rent-exemption without going through a real, rent-enforced
   transfer) and fixed it with `set_account_lamports`.
6. **`tests/stack-safety/`** ✅ DONE (compile-time asserts; the CLI-process
   bullet deferred to section 13) — 2 fail + 2 pass `trybuild` fixtures,
   confirming both the per-field and aggregate stack-frame budget asserts.
7. **`tests/token/`**, **`tests/cpi/`**, **`tests/events/`** — core
   functionality, but lower bug-density than 1-6 since they're more
   heavily used already (via examples) and less structurally tricky.
8. **`tests/idl-and-clientgen/`**, **`tests/instruction-attrs/`**,
   **`tests/error-codes/`** — round out coverage once the above exist.
9. **`tests/cli/`**, **`tests/nacvm/`** — different testing shape entirely
   (process/integration tests, not on-chain program tests); worth its own
   follow-up plan rather than bolting onto this one.
10. **`tests/optional-accounts/`** ✅ DONE — 40/40 passing across all 3 modes.
    Five real framework bugs found and fixed (`MUT_MASK`, remaining-accounts
    duplicate check, `naclac-syn`'s disconnected optional-detection, missing
    client-gen sentinel-fill, `no-std`/`pinocchio`/`alloc` routing), plus
    `init`/`close`/`realloc`/seeded-`bump` support added for `Option<T>`
    fields (previously rejected outright).

## Wiring into CI

Once even one case from the priority list exists, add a `test-programs` job
to `.github/workflows/ci.yml` — matrix over `tests/*`, `cd` in, run the
mode-appropriate build/test command per sub-crate. Don't wire this in before
any cases exist; an empty matrix job is noise. See Anchor's
`reusable-tests.yaml`'s `test-programs` job for the shape to copy (matrix of
`{cmd, path}`, not one job per case).
