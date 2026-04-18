# Design: Tempo → Magnus rename and folder restructure

**Date:** 2026-04-18
**Branch target:** `refactor/magnus-rename` (off `main`)
**Status:** Approved and implemented (pushed to `Magnus-Foundation/magnus`
`refactor/magnus-rename` on 2026-04-19)
**Scope:** Mechanical rename + folder restructure only. No new functionality.
**Related:** `transfer-station/design.md`, `transfer-station/1-week-sprint.md`,
`transfer-station/implementation-plan.md` (Magnus L1 product design — out of
scope here; that work lands in follow-up specs once the rename is merged)

## Goal

Rename the Tempo codebase to Magnus and reorganize the workspace into a
topology that matches the Magnus L1 subsystem boundaries described in
`transfer-station/design.md`. Preserve all existing behavior. Leave clearly
marked empty slots for Magnus-specific subsystems (Payment Engine, Gateway /
MGP, Bridge / MBS) that will be built in follow-up phases.

The refactor must be:

- **Purely mechanical** — zero behavioral change, zero dependency bumps, zero
  chain-state changes.
- **Phased and compile-verifiable** — each of the 8 phases ends on a known
  compile state, so any regression is bisectable to a single phase.
- **History-preserving** — directory moves use `git mv` so `git log --follow`
  keeps working on every file.

## Upstream relationship

**This rename is a hard fork from upstream Tempo (`tempoxyz/tempo`).** At the
time of this spec, `origin` pointed at upstream and local HEAD equaled upstream
HEAD (`786c8ce34`). No custom work had diverged yet.

After the rename lands:

- `origin` is **repointed** to `Magnus-Foundation/magnus` (completed).
- Upstream Tempo is **not** tracked. No `upstream` remote, no automatic merges
  from `tempoxyz/tempo`, no scripted rename-on-merge.
- The `reth-auto-bump` automation (currently a Tempo-side workflow) becomes
  Magnus's responsibility. Reth version bumps are owned by the Magnus team from
  merge day forward.
- Same for Commonware version tracking and any other upstream-sync automation.

**Attribution is preserved:**

- `LICENSE-MIT` and `LICENSE-APACHE` stay verbatim with Tempo/Stripe copyright
  lines intact. Magnus additions get their own copyright lines appended, never
  substituted.
- `README.md` gains a "Heritage" section crediting Tempo as the upstream the
  code forked from, with a link to `tempoxyz/tempo` at the fork SHA.
- `CHANGELOG.md` gains a top entry describing the rename and pointing at the
  fork SHA.

## Non-goals

- No Gateway precompile, Payment Engine, DEX seeding, demo wallet, or any
  Phase-0 Magnus functionality from `1-week-sprint.md`.
- No chain ID change (would invalidate signatures and E2E fixtures — belongs
  with the Phase-0 genesis work).
- No upstream dependency bumps (Reth, Commonware, alloy, revm stay pinned).
- No rewrite of historical CHANGELOG entries.
- No reformatting, clippy cleanup, or test additions beyond what the rename
  directly touches.
- No change to consensus NAMESPACE, RPC method namespaces, or Solidity keccak
  domain separators (all behavioral; deferred to chain-ID-change spec).

## Decisions (locked)

| # | Decision | Value |
|---|----------|-------|
| 1 | Scope | Rename + folder restructure, no new functionality |
| 2 | Crate prefix | `magnus-*` (1:1 replacement of `tempo-*`) |
| 3 | Binary name | `magnus` (plus `magnus-bench`, `magnus-sidecar`) |
| 4 | Folder topology | Grouped by responsibility (see below) |
| 5 | Git history | `git mv` for every directory move |
| 6 | Chain ID | **Unchanged** (defer new Magnus chain ID to Phase-0 spec) |
| 7 | Magnus subsystem slots | Pre-create `payments/`, `gateway/`, `bridge/` with README stubs pointing to the relevant `design.md` section |
| 8 | `contracts/` position | Under `evm/` (current revm coupling preserved) |
| 9 | TIP → MIP | Full rewrite: directory, filenames, internal references. Numbering preserved (tip-1000 → mip-1000, etc.) |
| 10 | Branch strategy | Single feature branch `refactor/magnus-rename`, single PR, one commit per phase (plus fixup commits as learnings emerge) |
| 11 | Top-level `tempo/` directory | Empty untracked skeleton from a previous aborted operation — deleted outright in Phase 1 |
| 12 | Fork strategy | **Hard fork.** No upstream tracking after merge. `origin` repointed to `Magnus-Foundation/magnus` in Phase 8. Attribution preserved (LICENSE, README heritage section, CHANGELOG entry). |

## Folder topology

```
magnus/  (was tempo/)
├── bin/
│   ├── magnus/               ← from bin/tempo
│   ├── magnus-bench/         ← from bin/tempo-bench
│   └── magnus-sidecar/       ← from bin/tempo-sidecar
│
├── crates/
│   ├── consensus/            ← Commonware Simplex + DKG + validator set
│   │   ├── consensus/              (was crates/consensus)
│   │   ├── commonware-node/        (was crates/commonware-node)
│   │   ├── commonware-node-config/ (was crates/commonware-node-config)
│   │   ├── dkg-onchain-artifacts/  (was crates/dkg-onchain-artifacts)
│   │   └── validator-config/       (was crates/validator-config)
│   │
│   ├── evm/                  ← revm stack, precompiles, on-chain contracts
│   │   ├── evm/                    (was crates/evm)
│   │   ├── revm/                   (was crates/revm)
│   │   ├── precompiles/            (was crates/precompiles)
│   │   ├── precompiles-macros/     (was crates/precompiles-macros)
│   │   └── contracts/              (was crates/contracts)
│   │
│   ├── node/                 ← node runtime + mempool
│   │   ├── node/                   (was crates/node)
│   │   └── transaction-pool/       (was crates/transaction-pool)
│   │
│   ├── payload/              ← block building (already nested in Tempo)
│   │   ├── builder/
│   │   └── types/
│   │
│   ├── primitives/           ← shared types + chainspec
│   │   ├── primitives/             (was crates/primitives)
│   │   ├── alloy/                  (was crates/alloy)
│   │   └── chainspec/              (was crates/chainspec)
│   │
│   ├── payments/             ← EMPTY + README. Payment Engine (design.md §Core Components 2)
│   ├── gateway/              ← EMPTY + README. MGP (design.md §Core Components 3)
│   ├── bridge/               ← EMPTY + README. MBS (design.md §Core Components 4)
│   │
│   ├── util/                 ← cross-cutting helpers
│   │   ├── eyre/                   (was crates/eyre)
│   │   ├── ext/                    (was crates/ext)
│   │   └── telemetry-util/         (was crates/telemetry-util)
│   │
│   └── tools/                ← dev/test infrastructure
│       ├── faucet/                 (was crates/faucet)
│       └── e2e/                    (was crates/e2e)
│
├── mips/                     ← was tips/ (full rewrite: mip-1000.md etc.)
├── contrib/
├── scripts/
├── xtask/
├── Cargo.toml                ← [workspace] members + [workspace.dependencies] rewritten
├── Cargo.lock
├── AGENTS.md                 ← Magnus branding
├── README.md                 ← rewritten for Magnus (Heritage section added)
├── CHANGELOG.md              ← rename entry at top (new file at root)
├── CNAME                     ← rustdocs.magnus.xyz
└── Justfile
```

## Rename surface

Every `tempo`/`Tempo`/`TEMPO`/`TIP` reference in the repo falls into one of
these buckets. The rename is exhaustive within these buckets.

| Bucket | Pattern → rename |
|---|---|
| Workspace packages | `tempo-node` → `magnus-node`, etc. (all 22 crates) |
| Binaries | `bin/tempo` → `bin/magnus`, plus `-bench`, `-sidecar` |
| Rust module paths | `tempo_node::…` → `magnus_node::…` |
| Rust type names (PascalCase) | `TempoNode`, `TempoChainSpec`, `TempoEvmConfig`, `TempoPayloadBuilder` → `Magnus*` |
| Rust constants (SCREAMING_SNAKE) | `TEMPO_SHARED_GAS_DIVISOR`, `TEMPO_T1_BASE_FEE`, `TEMPO_EXPIRING_NONCE_KEY`, etc. → `MAGNUS_*` |
| Feature flags | `tempo-*` → `magnus-*`; `cfg(feature = "tempo-…")` → `"magnus-…"` |
| Macros | `tempo_*!(...)` → `magnus_*!(...)` |
| Improvement proposals | `tips/tip-N.md` → `mips/mip-N.md`; "TIP-" → "MIP-"; "Tempo Improvement Proposal" → "Magnus Improvement Proposal" |
| Env vars | `TEMPO_*` → `MAGNUS_*` |
| Config files | `tempo.toml`, `tempo.nu` → `magnus.toml`, `magnus.nu` |
| Data dirs (in code) | `.tempo/` → `.magnus/` |
| CLI banners / help | "Tempo — …" → "Magnus — …" |
| Chainspec name string | `"tempo"` (network name, separate from chain ID) → `"magnus"` |
| Docker / Bake | `Dockerfile`/`Dockerfile.chef` labels, `docker-bake*.hcl` target names |
| Installer dir | `tempoup/` → `magnusup/` |
| CI workflows | `.github/workflows/*.yml` job names, artifact names, docker tags |
| Docs | `README.md`, `AGENTS.md`, `CNAME`, inline doc comments, LICENSE headers |
| Tooling | `Justfile` recipes, `scripts/*`, `xtask/` subcommand names |

**Exclusions (not renamed):**

- Upstream crate names: `reth-*`, `alloy-*`, `commonware-*`, `revm`.
- External git URLs (`paradigmxyz/reth`, etc.).
- Historical `CHANGELOG.md` entries from before this refactor.
- Historical PR references, commit SHAs, issue numbers.
- License attribution text.
- User data paths baked into deployed nodes (operator migration concern).

## Execution plan (8 phases, one commit per phase + fixups)

Each phase ends on a `cargo check --workspace --all-targets` state documented
in the gate column. No phase advances until its gate passes.

### Phase 1 — Scaffold new topology

- Create `crates/payments/`, `crates/gateway/`, `crates/bridge/` with a README
  stub each pointing to the relevant section of `transfer-station/design.md`.
- Add `.keep` placeholders to `crates/util/` and `crates/tools/` (which will
  receive crate moves in phase 2).
- Delete the top-level `tempo/` directory (empty skeleton from a previous
  aborted operation, zero files).
- **Gate:** `cargo check --workspace` unchanged.

### Phase 2 — Move crates via `git mv` (names still `tempo-*`)

- For each existing crate, `git mv` it under its new parent per the topology.
- Update `[workspace] members` and `[workspace.dependencies]` paths in root
  `Cargo.toml`.
- Package names stay `tempo-*` for now.
- Collision-name crates (`consensus`, `evm`, `node`, `primitives`) use a
  `_name_tmp` rename + `mkdir` + move-into-parent sequence because the dir
  that holds them must become the new parent.
- Three `include_str!` relative paths fixed as a consequence of the moves
  (test genesis fixtures crossing moved boundaries).
- **Gate:** `cargo check --workspace` passes; `cargo metadata` lists crates at
  new paths.

### Phase 3 — Rename package names in Cargo.toml files

- Root `Cargo.toml`: rewrite `[workspace.dependencies]` keys
  `tempo-*` → `magnus-*`.
- Each leaf `Cargo.toml`: rewrite `[package] name` and every internal crate
  reference using pattern `\btempo-([a-z][a-z0-9-]*)`.
- Rust source still imports `tempo_node::…`, so **this phase is expected to
  fail `cargo check`.** That's the signal phase 4 is needed.
- **Gate:** `cargo check --workspace` fails cleanly with unresolved-import
  errors only.

### Phase 4 — Rename Rust module paths

- Global scripted replace across `*.rs` files: pattern
  `\btempo_(?=[A-Za-z0-9_])` → `magnus_`.
- Five source files renamed on disk so `mod` declarations resolve:
  - `bin/tempo/src/tempo_cmd.rs` → `magnus_cmd.rs`
  - `crates/evm/precompiles/benches/tempo_precompiles.rs` → `magnus_precompiles.rs`
  - `crates/node/transaction-pool/src/tempo_pool.rs` → `magnus_pool.rs`
  - `crates/primitives/primitives/src/transaction/tempo_transaction.rs` → `magnus_transaction.rs`
  - `crates/primitives/primitives/src/reth_compat/transaction/tempo_transaction.rs` → `magnus_transaction.rs`
- `crates/node/node/tests/it/tempo_transaction/` directory renamed to
  `magnus_transaction/`.
- `[[bench]] name = "tempo_precompiles"` → `"magnus_precompiles"` in
  `crates/evm/precompiles/Cargo.toml`.
- **Gate:** `cargo check --workspace` and `cargo build --workspace` pass.

### Phase 5 — Rename Rust identifiers (types AND constants)

Two related rename categories in one commit:

- **PascalCase types:** pattern `\bTempo(?=[A-Za-z0-9_])`. ~121 unique
  identifiers: `TempoNode`, `TempoChainSpec`, `TempoEvmConfig`,
  `TempoPayloadBuilder`, `TempoConsensus`, `TempoEthApi`, `TempoArgs`,
  `TempoCli`, `TempoBench`, etc.
- **SCREAMING_SNAKE constants:** pattern `\bTEMPO_(?=[A-Z0-9_])`. 29 unique
  identifiers: `TEMPO_T1_BASE_FEE`, `TEMPO_SHARED_GAS_DIVISOR`,
  `TEMPO_EXPIRING_NONCE_KEY`, `TEMPO_SYSTEM_TX_SIGNATURE`, etc.

Both patterns use identifier boundaries so prose "Tempo" inside doc comments
and string literals that aren't immediately followed by an identifier char is
left for phase 7. Chain-tip method names (`tip_timestamp`, `tip_hash`, etc.)
are lowercase and unaffected — those are upstream Reth trait methods about
the chain HEAD, not Magnus Improvement Proposals.

- **Gate:** `cargo check --workspace --all-targets` passes.

### Phase 6 — Binaries, features, CLI, config, env

- `bin/tempo*` → `bin/magnus*` via `git mv`.
- Binary package/target names in `bin/magnus/Cargo.toml`: `name = "tempo"` →
  `"magnus"`, `default-run = "tempo"` → `"magnus"`.
- `bin/magnus/src/main.rs`: `.about("Tempo")` → `.about("Magnus")`,
  `default_value = "tempo"` for pyroscope app-name → `"magnus"`.
- `crates/node/node/src/version.rs`: `name_client: Cow::Borrowed("Tempo")` →
  `"Magnus"` (the node's client name in P2P handshakes + `web3_clientVersion`).
- `TEMPO_*` env vars / shell variables in `scripts/*.sh`, `tempoup/*`,
  `crates/util/ext/README.md` → `MAGNUS_*`.
- `tempo.nu` → `magnus.nu` with tempo/Tempo/TEMPO_ tokens rewritten.
- `Cargo.toml` workspace.members paths updated to `bin/magnus*`.

Deliberately kept for later phases / out of scope:
- RPC namespace attributes `#[rpc(namespace = "tempo")]` — behavioral API.
- Test invocations `parse(&["tempo", …])` — phase 7 prose sweep.
- URL literals like `rpc.moderato.tempo.xyz` — phase 7.

- **Gate:** `cargo check --workspace --all-targets` passes.

### Phase 7 — TIP → MIP and prose/docs/branding

- `git mv tips mips`; 23 `tip-N.md` → `mip-N.md` with content rewritten
  (TIP → MIP, Tempo → Magnus, "Tempo Improvement Proposal" → "Magnus …").
- `mips/ref-impls/` Solidity reference impls: `TIP20.sol` → `MIP20.sol`,
  `ITIP20.sol` → `IMIP20.sol`, `ITIP20Factory.sol` → `IMIP20Factory.sol`,
  `TIP403Registry.sol` → `MIP403Registry.sol`, etc.
- `mips/ref-impls/tempo-forge` → `magnus-forge`, `mips/ref-impls/tempo-cast`
  → `magnus-cast`; shell wrapper contents swept.
- `mips/ref-impls/lib/tempo-std` submodule local path → `magnus-std`;
  submodule URL intentionally unchanged (external `tempoxyz/tempo-std`).
- `.gitmodules` section labels updated `tips/ref-impls` → `mips/ref-impls`.
- Rust source: `ITIP20`/`ITIP403` → `IMIP20`/`IMIP403`; `TIP20` constants →
  `MIP20`; `TipFeeManager`/`TipFeeAMM` → `MipFeeManager`/`MipFeeAMM`. Rust
  module files `tip20.rs`/`tip20_factory.rs`/`tip403_registry.rs`/
  `tip_fee_manager.rs` and parallel directory names renamed to `mip*`.
- Solidity storage testdata fixtures renamed `tip20.sol`/`tip20.layout.json`/
  `tip20_factory.*`/`tip20_rewards_registry.*`/`tip403_registry.*` → `mip*`;
  Rust references to fixture paths updated.
- `scripts/create-tip20-token.sh` → `create-mip20-token.sh`; `scripts/Justfile`
  `create-tip20-token` recipe renamed.
- `crates/tools/faucet/Cargo.toml` description updated MIP20 tokens.
- Broad prose sweep across non-allowlist files: `Tempo`/`tempo`/`TIP`/
  `TIPs`/`Tempo Improvement Proposal` tokens rewritten.
- `tempoxyz/` org handle → `Magnus-Foundation/` in URLs.
- `README.md` gains a **Heritage section** crediting Tempo with the fork
  SHA and noting MIT/Apache-2.0 attribution preserved in LICENSE files.
- `CHANGELOG.md` created at repo root with an `Unreleased` rename entry.
- `AGENTS.md` `TIPs`/`tips/` references updated to `MIPs`/`mips/`.
- **Chain-tip method names** (`tip_timestamp`, `tip_hash`, `latest_tip`,
  `tip_block*`, `tip_header`) explicitly reverted — those are upstream Reth
  trait methods, not Magnus Improvement Proposals. This revert is the key
  lesson from the broader TIP sweep.

- **Gate:** `cargo check --workspace --all-targets` passes;
  `cargo doc --workspace --no-deps` builds.

### Phase 8 — CODEOWNERS, tempoup, NAMESPACE note, fork cutover

- `tempoup/` → `magnusup/` (git mv + inner `tempoup` script → `magnusup`);
  `TEMPOUP`/`tempoup`/`Tempoup` tokens rewritten in install scripts + README.
- `.github/CODEOWNERS` rewritten for the new workspace topology:
  `bin/tempo*` → `bin/magnus*`, flat crate paths → nested paths
  (`crates/primitives/alloy`, `crates/consensus/commonware-node`, etc.),
  `tips/` → `mips/`. Pre-refactor reviewers kept as a starting point;
  Magnus-Foundation handles assigned when org setup completes. Blank-owner
  entries for `crates/{payments,gateway,bridge}` mark them as Magnus-specific
  slots awaiting first contribution.
- `crates/consensus/commonware-node/src/config.rs` NAMESPACE constant gains
  an inline NOTE comment explaining why `b"TEMPO"` is intentionally preserved
  (consensus-layer domain separator; changing breaks network compatibility;
  deferred to chain-ID-change spec).
- `mips/ref-impls/test/invariants/` `TEMPO-{N|DIGIT}` invariant test labels
  renamed to `MAGNUS-{N|DIGIT}` (test naming convention, not signed strings).
- `scripts/test-cli.sh` `TEMPO` shell variable → `MAGNUS`.

Post-merge repo-admin (not commit-level):
- `origin` already repointed to `Magnus-Foundation/magnus`.
- Delete any `origin/reth-auto-bump` etc. upstream remote-tracking refs.
- Rotate shared CI secrets.
- Operator data-dir migration (`.tempo/` → `.magnus/`) in release notes.

- **Gate:** final-acceptance criteria below; CI green.

## Final acceptance criteria

1. `cargo check --workspace --all-features` passes clean.
2. `cargo build --workspace --release` passes.
3. `cargo test --workspace --lib` passes (1595 tests passed across 21 lib
   binaries in the reference run).
4. `cargo clippy --workspace --all-targets -- -D warnings` passes.
5. `cargo fmt --check` passes.
6. `cargo doc --workspace --no-deps` builds with zero warnings.
7. `rg -i '\btempo\b'` returns only allowlisted matches (see below).
8. `rg '\bTIP-?\d'` returns zero matches outside the allowlist.
9. Binary smoke test: `just magnus-dev-up` starts a node and produces blocks;
   `web3_clientVersion` reports `magnus/…`.
10. CI pipeline green on `refactor/magnus-rename`.

## Intentional allowlist (deliberate residual references)

Five references in the Rust/Solidity source are kept verbatim because changing
them breaks network/signature compatibility; all revisit with the chain-ID
change in a follow-up spec:

1. `crates/consensus/commonware-node/src/config.rs:49` —
   `pub const NAMESPACE: &[u8] = b"TEMPO"` (consensus-layer domain separator;
   inline NOTE comment above).
2. `crates/evm/precompiles/src/validator_config_v2/dispatch.rs:192` — comment
   describing `"TEMPO"` prefix format in the signed-message layout.
3. `mips/ref-impls/src/interfaces/IValidatorConfigV2.sol:158` —
   `keccak256(abi.encodePacked("TEMPO", "_VALIDATOR_CONFIG_V2_ADD_VALIDATOR", …))`.
4. `mips/ref-impls/src/interfaces/IValidatorConfigV2.sol:195` — same, rotate
   variant.
5. `mips/ref-impls/lib/magnus-std/scripts/sync.sh:25` — external submodule
   URL `https://github.com/tempoxyz/tempo.git` (hard-fork policy preserves
   external-dep URLs).

Paths whitelisted by design:
- `target/`, `.git/`, `Cargo.lock` (generated)
- `CHANGELOG.md` (rename entry)
- `README.md` Heritage section (intentional attribution)
- `transfer-station/*.md` (pre-refactor design docs authored as "Tempo fork")
- `docs/superpowers/specs/*.md` and `plans/*.md` (spec + plan)
- `LICENSE-MIT`, `LICENSE-APACHE` (attribution preserved verbatim)
- `mips/ref-impls/lib/` (external submodules: `forge-std`, `magnus-std`
  pointing at `tempoxyz/tempo-std`, `solady`)
- `.gitmodules` (submodule section labels)

## Safety nets (in-flight)

- **Compile after every phase.** Any regression is bisectable to a single
  phase.
- **Scripted pattern review.** Phases 3–7 are driven by `perl -i -pe` one-
  liners with carefully scoped patterns. Each phase's commit message shows
  the exact pattern used.
- **Grep allowlist audit.** Before PR, enumerate `rg` output for each
  allowlisted pattern and paste into the PR description.
- **Commit-message convention.** Each `refactor(phase-N)` commit states "no
  behavioral change" in its body. If a test starts failing, that claim is
  the first thing to re-examine.
- **Rollback.** Everything on `refactor/magnus-rename`. Post-merge regressions
  revert in reverse phase order on `main`.

## Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Phase 5 pattern catches `Tempo` inside prose doc comments | Low | Identifier-boundary lookahead; phase 7 catches the prose remainder |
| Broad sweep misses `ITIP`/`TIP20` interior matches | Caught | Fixup commit with explicit token list (`tip20`, `tip20_factory`, `tip403_registry`, `tip_fee_manager`, `tip_fee_amm`, `TipFeeManager`, `TipFeeAMM`, `TIP20`, `ITIP20`) |
| Broad sweep over-renames chain-tip method names | Caught | Explicit revert in phase 7 for `tip_timestamp`/`tip_hash`/`latest_tip`/`tip_block*`/`tip_header` |
| Future Reth/Commonware bumps land on Magnus without upstream's automation | Certain (post-merge) | Hard fork is a knowing tradeoff; `reth-auto-bump` workflow retargeted in phase 8 |
| Attribution requirement missed (MIT/Apache-2.0) | Low | LICENSE files stay verbatim; Heritage section in README |
| Chain ID accidentally changes through a missed constant | Low | Binary smoke test; if chain ID shifted, signature verification fails immediately |
| Operator data-dir path change breaks running nodes | N/A (refactor branch) | Migration note in release notes |

## Lessons learned (baked into plan)

The first execution pass produced these fixup commits that are now merged
into the phase design above:

- Phase 2's original two-step `git mv` sequence needed an intermediate `mkdir`
  for the four collision-name crates. Documented.
- Phase 4 missed `[[bench]]` target names (snake_case, not hyphenated). Added
  to phase 4 scope.
- Phase 5's SCREAMING_SNAKE pattern wasn't in the original spec; only added
  after grep survey. Now first-class.
- Phase 7's `\bTIP\b` standalone-word pattern missed `TIP20`/`TIP403` in Rust
  because those have digits after. Explicit token list added.
- Phase 7's broad `\btip(?=[0-9_])` pattern over-renamed Reth chain-tip
  methods. Explicit revert list added.

## Execution reference

Implemented on `refactor/magnus-rename` in 11 commits (9 refactor phases +
docs restore + rustfmt style), pushed to `Magnus-Foundation/magnus` on
2026-04-19. Final head `cc71bc8a6`. 1595 library tests pass;
`cargo check --workspace --all-targets` passes; `just magnus-dev-up`
produces blocks and reports `web3_clientVersion` as `magnus/…`.
