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
| 2 | **solana-zerocopy** | default `solana` backend, `borsh` feature **off** | `Account<T>` aliased to `AccountLoader<T>` | `Pod`/`bytemuck`, in-place |
| 3 | **pinocchio** | `pinocchio` feature on (implies zero-copy, no_std) | `Account<T>` aliased to `AccountLoader<T>` | `Pod`/`bytemuck`, in-place |

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

## 2. `tests/accounts-constraints/` — every `#[account(...)]` constraint, individually

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

**🚧 Core subset built, not yet run.** `tests/accounts-constraints/` exists
(3 modes: `accounts_constraints`, `accounts_constraints_borsh`,
`accounts_constraints_zc`) with one program covering:

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

Deferred to a follow-up case (isolated on purpose — these touch raw SPL
byte-layout parsing and have their own footguns, confirmed via direct
`security.rs` code reading, not guessed):
- `token::mint` / `token::authority` / `token::program` — read mint/authority
  bytes directly out of the SPL token account's raw data at fixed offsets
  (`[0..32]`/`[32..64]`); mismatch errors are inconsistent
  (`ConstraintAccountIsNone` for `mint`, `ConstraintAddress` for `authority`,
  `ProgramIdMismatch` for the owner-based `program` check) — worth its own
  case specifically to pin down that inconsistency.
- `mint::decimals` / `mint::authority` / `mint::freeze_authority` — same
  fixed-offset SPL layout reads; `decimals` mismatch returns the generic
  `Unauthorized` rather than a constraint-specific error.
- `realloc` — confirmed via code reading that shrinking issues **no lamport
  refund** and `realloc::zero` is parsed but **never actually applied**
  (`let _zero_flag = ...` — dead code); both are real gaps worth a dedicated
  regression test once this lands, not just a note here.
- `executable` — not yet built in the core pass; straightforward to add
  (pass a non-executable account into an `executable`-required slot).

**Note:** don't build 20 separate example programs for this — one program
per mode with ~20 instructions (one per constraint) keeps compile time sane
while still isolating failures per constraint.

---

## 3. `tests/pda-seeds/` — the seed-expression classification bug class ✅ DONE

**Modes:** solana-zerocopy and pinocchio only (this is specifically about
the `AccountLoader<T>`/`Account<T>`-as-zero-copy `Deref` rewrite path;
solana-borsh doesn't hit this code path at all).

This is the area the audit found live bugs in twice (#3, and Part 6 of the
consistency plan) — the kind of code that's proven itself prone to silent
regressions. Five instructions (`init_registry`, `init_entry`,
`touch_entry_bare_bump`, `touch_registry_explicit_bump`, `init_child`) cover:

- Literal seed + bare bump on `init` (compile-time-precomputed PDA path —
  `security.rs`'s `resolve_seed_literal` textually reads `constants.rs` off
  disk at macro-expansion time and embeds the resolved bytes directly; the
  `use` import for the seed constant is genuinely unused in the expanded
  output, not a false-positive warning)
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

## 4. `tests/dup-mut/` — duplicate mutable account aliasing

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

## 6. `tests/token/` — SPL / pinocchio-token parity

**Modes:** solana-borsh, solana-zerocopy, pinocchio (token backend selection
follows the same 3-way split via the unified `token` feature).

- Create a token account via `token::` constraints, transfer, check balance
- `AssociatedToken` derivation and creation (`init` + ATA constraint)
- Token-2022 variant specifically (separate from legacy `spl-token`) — extensions if any are supported, otherwise just confirm the base path works under the 2022 program ID
- Mint creation with `mint::decimals`/`mint::authority`/`mint::freeze_authority`
- CPI into token program from within an instruction (`ToCpiHandle`/`ToCpiHandleMut` path — `naclac-token/src/token.rs`)

**Note:** confirm this actually runs against real SPL Token program logic
(via `litesvm`, which is already a workspace dependency per root `Cargo.toml`)
rather than mocking the CPI — a mocked token CPI would defeat the point of
testing token constraint enforcement.

---

## 7. `tests/events/` — event emission across all three event mechanisms

**Modes:** all three (`naclac-core/src/event/{borsh,zero_copy,pinocchio}.rs`
are three genuinely different implementations, not one implementation with
cfg branches).

- Emit an event, decode it back from the transaction's log output, confirm
  field values round-trip correctly in each of the three backends
- Confirm the discriminator prefix on the emitted event matches what the
  client-gen'd decoder expects (this is the join point with `tests/client-gen/`
  below — an event encoding bug would show up as silent decode failure on
  the generated client, not a framework-side error)

---

## 8. `tests/cpi/` — cross-program invocation

**Modes:** all three.

- CPI to the System Program (`system_program.rs` — `SystemTransferAccounts`,
  `CreateAccountAccounts`)
- CPI to a second naclac-built program (self-CPI style — the pattern
  `#[cpi]` on `naclac-macros/src/lib.rs:174` generates client stubs for)
- Signed CPI with PDA seeds (`invoke_signed` / `invoke_signed_pinocchio`) —
  confirm `MAX_CPI_SIGNERS`/`MAX_CPI_SEEDS_PER_SIGNER` stack-allocation
  limits (`naclac-lang/src/prelude.rs:56-60`) are actually enforced with a
  clear error, not silent truncation, when exceeded

---

## 9. `tests/stack-safety/` — the `Box<Account<T>>` boxing mechanism

**Modes:** solana-borsh only (zero-copy is explicitly out of scope for this
mechanism per the stack-overflow fix plan — `AccountLoader<T>` doesn't embed
the full struct inline, so it isn't subject to the same frame-size math).

- A struct whose data-carrying fields sum under the per-field/aggregate
  budget — confirm it compiles with **no** boxing required
- A struct with one field over the per-field budget — confirm the
  `const _: () = assert!(...)` fires at compile time with a message naming
  that field, and that wrapping it in `Box<Account<T>>` clears the error
- A struct with several fields each under budget individually but over
  budget in aggregate — confirm the aggregate assert fires (this is the
  case the per-field check alone can't catch)
- Confirm `Box<Account<T>>` is a fully transparent field: `ctx.accounts.foo.some_method()`,
  `ctx.accounts.foo.address()`, and using it as a CPI handle source all work
  unchanged, per the blanket impls added for `NaclacAccount`/`Owner`/
  `ToAddress`/`AsRefByteSlice`/`ToAccountInfo`/`ToAccountInfos`/`ToCpiHandle`/`ToCpiHandleMut`
- Actually trigger `naclac build` (not just `cargo check`) on an
  intentionally-oversized, unboxed struct and confirm the CLI now fails the
  command (per `build.rs`'s output-scanning fix) rather than reporting
  success despite the SBF stack-frame diagnostic

---

## 10. `tests/instruction-attrs/` — `close_account`, `realloc`, `init_cpi` codegen paths

**Modes:** solana-borsh and pinocchio (per the audit's confirmed note that
`close_account.rs`/`realloc.rs` correctly use a 2-way split, not 3-way —
Borsh and solana-zerocopy share `AccountInfo` mechanics on this path, so a
solana-zerocopy-specific case here would be redundant with the pinocchio
case's coverage of the "other" branch... **verify this assumption before
skipping solana-zerocopy entirely** — the audit confirmed it for the two
files named, not for every instruction-attr file).

- `close`: account closed mid-instruction, lamports go to the right
  destination, subsequent access in the same instruction is rejected
- `realloc` growing and shrinking, with and without `zero` (if a
  zero-on-realloc option exists — confirm from `realloc.rs`)
- `init` via CPI to system program (`init_cpi.rs`) for both a plain account
  and a PDA (seeds-derived) account

---

## 11. `tests/error-codes/` — `#[error_code]` and error propagation

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
`Account<T>`/`AccountLoader<T>` fields (the same technique already used for
`Program<T>`, just previously not applied to `Account<T>`), `pda.rs`
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

## Priority order (highest leverage first)

1. **`tests/discriminator/`** ✅ DONE — security property, cheap to write,
   three modes, 6/6 passing.
2. **`tests/pda-seeds/`** ✅ DONE — proven bug-prone area, previously-confirmed
   zero-coverage gap (auto-bump) now covered, 2/2 passing.
3. **`tests/compile-fail/`** ✅ DONE (14/15 sites; 15th needs a different
   testing shape, see section 5) — cheap per-site (`trybuild`, no
   litesvm/program compile needed). 4 legacy-argument checks were removed
   from the framework outright rather than tested (see section 5).
4. **`tests/dup-mut/`** — mechanism already exists and is used in production
   (`launchpad`'s `amm`), just needs the regression test written. Next up.
5. **`tests/accounts-constraints/`** — broadest surface, catches the most
   "this constraint silently does nothing" class of bug.
6. **`tests/stack-safety/`** — recently-built mechanism, no coverage yet at
   all per the stack-overflow fix plan's own admission.
7. **`tests/token/`**, **`tests/cpi/`**, **`tests/events/`** — core
   functionality, but lower bug-density than 1-6 since they're more
   heavily used already (via examples) and less structurally tricky.
8. **`tests/idl-and-clientgen/`**, **`tests/instruction-attrs/`**,
   **`tests/error-codes/`** — round out coverage once the above exist.
9. **`tests/cli/`**, **`tests/nacvm/`** — different testing shape entirely
   (process/integration tests, not on-chain program tests); worth its own
   follow-up plan rather than bolting onto this one.

## Wiring into CI

Once even one case from the priority list exists, add a `test-programs` job
to `.github/workflows/ci.yml` — matrix over `tests/*`, `cd` in, run the
mode-appropriate build/test command per sub-crate. Don't wire this in before
any cases exist; an empty matrix job is noise. See Anchor's
`reusable-tests.yaml`'s `test-programs` job for the shape to copy (matrix of
`{cmd, path}`, not one job per case).
