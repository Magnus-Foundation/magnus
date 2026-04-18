# Magnus Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the Tempo → Magnus mechanical rename + folder restructure
described in `docs/superpowers/specs/2026-04-18-magnus-rename-design.md`
without behavioral change.

**Architecture:** Hard fork from `tempoxyz/tempo` at SHA `786c8ce34`. Eight
phases, each ending in one commit that leaves `cargo check` in a known state
(passing, except phase 3 which is the designed-fail pivot). All directory
moves use `git mv` for history preservation. All scripted renames use
identifier-boundary `perl -pe` patterns committed alongside the phase.

**Tech Stack:** Rust 2024 edition, Commonware consensus crates, Reth/alloy/
revm (pinned), `perl -i -pe` for scripted pattern replacement.

---

## Task 0 — Setup

### 0.1 Verify clean working tree

- [ ] `git status` → no uncommitted changes.
- [ ] `git branch --show-current` → on `main`.
- [ ] `git log -1` → at the fork SHA (`786c8ce34` or equivalent).

### 0.2 Create feature branch

```bash
git checkout -b refactor/magnus-rename
```

### 0.3 Capture baseline

- [ ] `cargo check --workspace` passes. Record any warnings.
- [ ] `cargo metadata --format-version=1 --no-deps | jq -r '.packages[].name' | sort > /tmp/magnus-rename-packages-before.txt`
- [ ] `wc -l /tmp/magnus-rename-packages-before.txt` → expect 26.

### 0.4 Repoint remote (hard-fork cutover)

```bash
git remote set-url origin https://github.com/Magnus-Foundation/magnus.git
git remote -v
```

---

## Task 1 — Phase 1: scaffold Magnus topology

### 1.1 Create parent dirs + Magnus-specific slots

```bash
mkdir -p crates/payments crates/gateway crates/bridge crates/util crates/tools
touch crates/util/.keep crates/tools/.keep
```

**Note:** `crates/consensus/`, `crates/evm/`, `crates/node/`, and
`crates/primitives/` already exist as live crates. Adding `.keep` to them
would go inside the existing crate and is skipped. Their role as "new
parent dirs" emerges naturally during phase 2 via the `_name_tmp` dance.

### 1.2 Write three Magnus subsystem README stubs

Write `crates/payments/README.md`, `crates/gateway/README.md`,
`crates/bridge/README.md` (see spec for exact content). Each points at
`transfer-station/design.md` §Core Components.

### 1.3 Delete top-level `tempo/` directory

- [ ] Verify empty: `find tempo/ -type f` returns nothing and
  `git ls-files tempo/` returns nothing.
- [ ] `rm -rf tempo/`.

### 1.4 Verify + commit

- [ ] `cargo check --workspace` passes (no crates moved; should match baseline).
- [ ] Stage and commit:

```bash
git add -A
git commit -m "refactor(phase-1): scaffold magnus topology and remove empty tempo/ dir"
```

**Gate:** commit SHA, `git status` clean, `tempo/` gone, `cargo check` green.

---

## Task 2 — Phase 2: relocate crates with `git mv`

### 2.1 Move consensus-related crates (collision-name pattern)

```bash
git mv crates/consensus crates/_consensus_tmp
mkdir crates/consensus
git mv crates/_consensus_tmp crates/consensus/consensus

git mv crates/commonware-node          crates/consensus/commonware-node
git mv crates/commonware-node-config   crates/consensus/commonware-node-config
git mv crates/dkg-onchain-artifacts    crates/consensus/dkg-onchain-artifacts
git mv crates/validator-config         crates/consensus/validator-config
```

### 2.2 Move EVM-related crates

```bash
git mv crates/evm crates/_evm_tmp
mkdir crates/evm
git mv crates/_evm_tmp crates/evm/evm

git mv crates/revm                crates/evm/revm
git mv crates/precompiles         crates/evm/precompiles
git mv crates/precompiles-macros  crates/evm/precompiles-macros
git mv crates/contracts           crates/evm/contracts
```

### 2.3 Move node + transaction-pool

```bash
git mv crates/node crates/_node_tmp
mkdir crates/node
git mv crates/_node_tmp crates/node/node

git mv crates/transaction-pool crates/node/transaction-pool
```

### 2.4 Move primitives, alloy, chainspec

```bash
git mv crates/primitives crates/_primitives_tmp
mkdir crates/primitives
git mv crates/_primitives_tmp crates/primitives/primitives

git mv crates/alloy     crates/primitives/alloy
git mv crates/chainspec crates/primitives/chainspec
```

### 2.5 Move util crates

```bash
git mv crates/eyre           crates/util/eyre
git mv crates/ext            crates/util/ext
git mv crates/telemetry-util crates/util/telemetry-util
git rm crates/util/.keep
```

### 2.6 Move tools crates

```bash
git mv crates/faucet crates/tools/faucet
git mv crates/e2e    crates/tools/e2e
git rm crates/tools/.keep
```

### 2.7 Update workspace members and workspace.dependencies paths

Edit `Cargo.toml` root:

- `[workspace] members = [ ... ]` — replace list with the new nested paths.
  Binary paths (`bin/tempo*`) keep `tempo-` for now (renamed in phase 6).
- `[workspace.dependencies]` — each `path = "..."` updated to the new nested
  path. Package names stay `tempo-*` for now.

### 2.8 Fix `include_str!` paths broken by moves

Three test-fixture paths cross moved boundaries:

```bash
perl -i -pe 's|include_str!\("\.\./\.\./node/tests/assets/test-genesis\.json"\)|include_str!("../../../node/node/tests/assets/test-genesis.json")|g' \
  crates/tools/e2e/src/execution_runtime.rs
perl -i -pe 's|\.\./\.\./\.\./chainspec/src/genesis|../../../../primitives/chainspec/src/genesis|g' \
  crates/node/node/tests/it/eth_call.rs \
  crates/node/node/tests/it/utils.rs
```

### 2.9 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] `cargo metadata ... > /tmp/magnus-rename-packages-phase2.txt`;
  `diff` against baseline → empty (same 26 packages).
- [ ] Commit: `refactor(phase-2): relocate crates under grouped parent directories`.

---

## Task 3 — Phase 3: rename packages in Cargo.toml (expected-fail)

### 3.1 Scripted rewrite

```bash
find crates/ bin/ xtask/ -name Cargo.toml -type f -exec \
  perl -i -pe 's/\btempo-([a-z][a-z0-9-]*)/magnus-$1/g' {} +
perl -i -pe 's/\btempo-([a-z][a-z0-9-]*)/magnus-$1/g' Cargo.toml
```

### 3.2 Preserve bin/tempo* paths in workspace.members

The scripted pattern rewrites `"bin/tempo-bench"` → `"bin/magnus-bench"` in
the members list. Those dirs aren't renamed until phase 6, so paths must
revert:

```bash
perl -i -pe 's|"bin/magnus-|"bin/tempo-|g' Cargo.toml
```

### 3.3 Verify expected-fail + commit

- [ ] `cargo check --workspace` **fails** with
  `unresolved import tempo_*` errors only. Any other error class means the
  pattern escaped somewhere unintended.
- [ ] Commit: `refactor(phase-3): rename tempo-* packages to magnus-* in Cargo.toml`
  (commit message explicitly notes the expected-fail state).

---

## Task 4 — Phase 4: rename Rust module paths

### 4.1 Scripted rewrite

```bash
find bin/ crates/ xtask/ -name '*.rs' -type f -exec \
  perl -i -pe 's/\btempo_(?=[A-Za-z0-9_])/magnus_/g' {} +
```

### 4.2 Rename tempo_*.rs source files

```bash
for f in $(find bin/ crates/ xtask/ -type f \
  \( -name 'tempo_*.rs' -o -name 'tempo-*.rs' \)); do
  new=$(dirname "$f")/$(basename "$f" | sed "s|^tempo_|magnus_|; s|^tempo-|magnus-|")
  git mv "$f" "$new"
done
```

### 4.3 Rename `tempo_transaction/` test directory

```bash
git mv crates/node/node/tests/it/tempo_transaction \
       crates/node/node/tests/it/magnus_transaction
```

### 4.4 Rename `[[bench]]` target

```bash
perl -i -pe 's/^name = "tempo_precompiles"$/name = "magnus_precompiles"/' \
  crates/evm/precompiles/Cargo.toml
```

### 4.5 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] Commit: `refactor(phase-4): rename Rust module paths tempo_* to magnus_*`.

---

## Task 5 — Phase 5: rename Rust identifiers (types + SCREAMING_SNAKE)

### 5.1 Two-pattern scripted rewrite

```bash
find bin/ crates/ xtask/ -name '*.rs' -type f -exec perl -i -pe '
  s/\bTempo(?=[A-Za-z0-9_])/Magnus/g;
  s/\bTEMPO_(?=[A-Z0-9_])/MAGNUS_/g;
' {} +
```

Patterns are identifier-boundary so prose "Tempo" inside `///` doc comments
(followed by space/punctuation, not an identifier char) is left untouched for
phase 7. Chain-tip method names (`tip_timestamp`, etc.) are lowercase and
unaffected.

### 5.2 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] Commit:
  `refactor(phase-5): rename Rust identifiers (types + SCREAMING_SNAKE)`.

---

## Task 6 — Phase 6: binaries, CLI, env, config

### 6.1 Rename bin dirs

```bash
git mv bin/tempo          bin/magnus
git mv bin/tempo-bench    bin/magnus-bench
git mv bin/tempo-sidecar  bin/magnus-sidecar
```

### 6.2 Update workspace.members paths

```bash
perl -i -pe 's|"bin/tempo"|"bin/magnus"|;
             s|"bin/tempo-bench"|"bin/magnus-bench"|;
             s|"bin/tempo-sidecar"|"bin/magnus-sidecar"|' Cargo.toml
```

### 6.3 Update bin/magnus Cargo.toml

```bash
perl -i -pe 's/^name = "tempo"$/name = "magnus"/;
             s/^default-run = "tempo"$/default-run = "magnus"/' \
  bin/magnus/Cargo.toml
```

(The `[[bin]] name = "tempo"` entry is covered by the first substitution since
`name =` appears on its own line.)

### 6.4 Client name and CLI banner

```bash
perl -i -pe 's/Cow::Borrowed\("Tempo"\)/Cow::Borrowed("Magnus")/g' \
  crates/node/node/src/version.rs
perl -i -pe 's/\.about\("Tempo"\)/\.about("Magnus")/g;
             s/default_value = "tempo"/default_value = "magnus"/g' \
  bin/magnus/src/main.rs
```

### 6.5 Rename `tempo.nu` → `magnus.nu` and sweep contents

```bash
git mv tempo.nu magnus.nu
perl -i -pe 's/\btempo\b/magnus/g;
             s/\bTempo\b/Magnus/g;
             s/\bTEMPO_/MAGNUS_/g' magnus.nu
```

### 6.6 Sweep TEMPO_ in shell/ops files

```bash
for f in scripts/foundry-patch.sh \
         scripts/nightly-multi-node-benchmark.sh \
         scripts/nightly-single-node-benchmark.sh \
         tempoup/tempoup tempoup/install tempoup/README.md \
         crates/util/ext/README.md; do
  [ -f "$f" ] && perl -i -pe 's/\bTEMPO_/MAGNUS_/g' "$f"
done
```

### 6.7 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] Commit: `refactor(phase-6): rename binaries, CLI banner, env vars, config`.

---

## Task 7 — Phase 7: TIP → MIP + prose + docs + branding

### 7.1 Rename tips/ → mips/ and tip-N.md → mip-N.md

```bash
git mv tips mips
cd mips && for f in tip-*.md; do git mv "$f" "${f/tip-/mip-}"; done && cd ..
```

### 7.2 Rewrite mips/ content

```bash
find mips/ -type f \( -name '*.md' -o -name '*.sol' -o -name '*.toml' -o -name '*.txt' \) \
  ! -name 'LICENSE*' -exec perl -i -pe '
  s/Tempo Improvement Proposal/Magnus Improvement Proposal/g;
  s/\bTIP-(\d)/MIP-$1/g;
  s/\bTIP(?=\d)/MIP/g;
  s/\bTIP\b/MIP/g;
  s/\bITIP(?=[A-Za-z0-9_])/IMIP/g;
  s/\bTempo\b/Magnus/g;
  s/\btempo\b/magnus/g;
' {} +
```

### 7.3 Rename Solidity files in mips/ref-impls/

```bash
find mips/ \( -name 'TIP*' -o -name 'ITIP*' \) -type f | while read f; do
  new=$(dirname "$f")/$(basename "$f" | sed -E 's/^(I)?TIP/\1MIP/')
  git mv "$f" "$new"
done
git mv mips/ref-impls/lib/tempo-std mips/ref-impls/lib/magnus-std
git mv mips/ref-impls/tempo-forge   mips/ref-impls/magnus-forge
git mv mips/ref-impls/tempo-cast    mips/ref-impls/magnus-cast
```

Sweep the renamed shell wrappers:

```bash
perl -i -pe 's/\btempo-forge\b/magnus-forge/g;
             s/\btempo-cast\b/magnus-cast/g;
             s/\bTempo mode\b/Magnus mode/g;
             s/\btempo\b/magnus/g;
             s/\bTempo\b/Magnus/g;
             s/\bTEMPO_/MAGNUS_/g' \
  mips/ref-impls/magnus-forge mips/ref-impls/magnus-cast
perl -i -pe 's|tips/ref-impls|mips/ref-impls|g' .gitmodules
```

### 7.4 Rust TIP/tip identifier rename (targeted, not broad)

The broad `\btip(?=[0-9_])` pattern over-matches Reth chain-tip methods.
Use explicit token list:

```bash
find bin/ crates/ xtask/ -name '*.rs' -type f -exec perl -i -pe '
  s/\btip20_factory\b/mip20_factory/g;
  s/\btip403_registry\b/mip403_registry/g;
  s/\btip_fee_manager\b/mip_fee_manager/g;
  s/\btip_fee_amm\b/mip_fee_amm/g;
  s/\btip20\b/mip20/g;
  s/\bTIP20\b/MIP20/g;
  s/\bITIP20\b/IMIP20/g;
  s/\bTipFeeManager\b/MipFeeManager/g;
  s/\bTipFeeAMM\b/MipFeeAMM/g;
' {} +
```

### 7.5 Rename tip*.rs files and tip*/ dirs

```bash
for f in $(find bin/ crates/ xtask/ -type f \( -name 'tip*.rs' -o -name 'tip_*.rs' \)); do
  new=$(dirname "$f")/$(basename "$f" | sed 's/^tip/mip/')
  git mv "$f" "$new"
done
for d in $(find bin/ crates/ xtask/ -type d -name 'tip*'); do
  new=$(dirname "$d")/$(basename "$d" | sed 's/^tip/mip/')
  git mv "$d" "$new"
done
```

Fix `mod`/`use`/`crate::`/`self::` `tip\d` paths:

```bash
find bin/ crates/ xtask/ -name '*.rs' -type f -exec perl -i -pe '
  s/\bmod tip(\d|_)/mod mip$1/g;
  s/\buse tip(\d|_)/use mip$1/g;
  s/\bcrate::tip(\d|_)/crate::mip$1/g;
  s/\bself::tip(\d|_)/self::mip$1/g;
' {} +
```

### 7.6 Rename Solidity storage testdata fixtures

```bash
for f in crates/evm/precompiles/tests/storage_tests/solidity/testdata/tip*; do
  [ -f "$f" ] && git mv "$f" "$(dirname "$f")/$(basename "$f" | sed 's/^tip/mip/')"
done
find crates/evm/precompiles/tests/storage_tests/solidity/testdata/ -type f -name 'mip*' \
  -exec perl -i -pe 's/\bTIP20\b/MIP20/g;
                     s/\bITIP20\b/IMIP20/g;
                     s/\btip20\b/mip20/g;
                     s/\bTIP403\b/MIP403/g;
                     s/\btip403\b/mip403/g' {} +
rg -l 'testdata/tip' --type rust crates/ | xargs perl -i -pe 's|testdata/tip|testdata/mip|g'
```

### 7.7 Script rename

```bash
git mv scripts/create-tip20-token.sh scripts/create-mip20-token.sh
perl -i -pe 's/\bTIP20\b/MIP20/g; s/\btip20\b/mip20/g' \
  scripts/create-mip20-token.sh scripts/Justfile crates/tools/faucet/Cargo.toml
```

### 7.8 Revert over-renamed chain-tip methods

The broad tip sweep may have caught `tip_timestamp`, `tip_hash`, etc.
Explicit revert:

```bash
find bin/ crates/ xtask/ -name '*.rs' -exec perl -i -pe '
  s/\bmip_timestamp\b/tip_timestamp/g;
  s/\bmip_hash\b/tip_hash/g;
  s/\bmip_block\b/tip_block/g;
  s/\bmip_block_hash\b/tip_block_hash/g;
  s/\bmip_block_number\b/tip_block_number/g;
  s/\blatest_mip\b/latest_tip/g;
  s/\bmip_header\b/tip_header/g;
' {} +
```

### 7.9 Broad prose + tempoxyz sweep

```bash
rg -l '\btempo\b|\bTempo\b|\bTIP\b|\bTIP-\d|\bTIPs\b|tips/ref|tips/tip|/tips/tip|tip/XXXX' \
  --glob '!target/**' --glob '!mips/**' --glob '!CHANGELOG.md' \
  --glob '!LICENSE*' --glob '!Cargo.lock' --glob '!.git/**' \
  --glob '!.gitmodules' | xargs perl -i -pe '
    s/Tempo Improvement Proposal/Magnus Improvement Proposal/g;
    s/\bTIP-(\d)/MIP-$1/g;
    s/\bTIP(?=\d)/MIP/g;
    s/\bTIP\b/MIP/g;
    s/\bTIPs\b/MIPs/g;
    s|\btips/ref-impls|mips/ref-impls|g;
    s|\btips/tip-(\d)|mips/mip-$1|g;
    s|/tips/tip-(\d)|/mips/mip-$1|g;
    s|`tips/`|`mips/`|g;
    s|`tips/tip-XXXX\.md`|`mips/mip-XXXX.md`|g;
    s/\bTempo\b/Magnus/g;
    s/\btempo\b/magnus/g;
  '

rg -l 'tempoxyz' --glob '!target/**' --glob '!mips/**' --glob '!LICENSE*' \
  --glob '!Cargo.lock' --glob '!.git/**' --glob '!.gitmodules' | \
  xargs perl -i -pe 's|\btempoxyz/tempo\b|Magnus-Foundation/magnus|g;
                     s|\btempoxyz/magnus\b|Magnus-Foundation/magnus|g;
                     s|\btempoxyz\b|Magnus-Foundation|g'
```

### 7.10 Fix README stubs accidentally swept

The broad sweep may rewrite `Tempo → Magnus rename refactor` in README stubs
(making it nonsensical). Fix:

```bash
for f in crates/payments/README.md crates/gateway/README.md crates/bridge/README.md; do
  perl -i -pe 's/Magnus → Magnus rename refactor/Magnus rename refactor/g' "$f"
done
```

### 7.11 Add Heritage section to README and CHANGELOG entry

Append Heritage section to `README.md`:

```markdown
## Heritage

Magnus is a hard fork of [Tempo](https://github.com/tempoxyz/tempo) at SHA
`786c8ce34`. The Tempo project — created by Stripe with consensus work by
Commonware — provided the foundation for consensus, EVM integration, and
the overall node architecture. Tempo and Stripe copyright lines remain in
[`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE) per
the MIT/Apache-2.0 attribution requirements. Magnus extends this foundation
with payments, gateway, and bridge subsystems described in the project's
design docs.
```

Create `CHANGELOG.md` at repo root with `## Unreleased` entry describing the
rename and pointing at the fork SHA.

### 7.12 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] `rg -i '\btempo\b'` outside allowlist returns only Heritage +
  CHANGELOG + intentional residual refs.
- [ ] Commit:
  `refactor(phase-7): rename tips to mips, sweep prose, docs and branding`.

---

## Task 8 — Phase 8: CODEOWNERS, tempoup, NAMESPACE note, fork cutover

### 8.1 Rename tempoup/

```bash
git mv tempoup magnusup
git mv magnusup/tempoup magnusup/magnusup
perl -i -pe 's/TEMPOUP/MAGNUSUP/g;
             s/\btempoup/magnusup/g;
             s/\bTempoup/Magnusup/g;
             s/tempoup/magnusup/g' \
  magnusup/magnusup magnusup/install magnusup/README.md
```

### 8.2 Rewrite CODEOWNERS for new topology

Edit `.github/CODEOWNERS` — paths for nested crates, `bin/magnus*`,
`mips/`, blank-owner entries for `crates/{payments,gateway,bridge}`.

### 8.3 Add NAMESPACE NOTE

Prepend to `crates/consensus/commonware-node/src/config.rs` NAMESPACE
declaration:

```rust
// NOTE: NAMESPACE is a consensus-layer domain separator used in cryptographic
// signing/verification. Changing it breaks network compatibility. Kept as
// "TEMPO" for the Magnus rename refactor to preserve behavior; revisit with
// the chain-ID change in a follow-up spec.
pub const NAMESPACE: &[u8] = b"TEMPO";
```

### 8.4 Sweep invariant test labels + test-cli.sh

```bash
find mips/ref-impls/ -type f \( -name '*.t.sol' -o -name '*.sol' -o -name '*.md' \) \
  ! -path '*/lib/*' -exec perl -i -pe '
  s/\bTEMPO-([A-Z])/MAGNUS-$1/g;
  s/\bTEMPO-(\d)/MAGNUS-$1/g;
  s/\bTEMPO TRANSACTION\b/MAGNUS TRANSACTION/g;
' {} +

perl -i -pe 's/\bTEMPO\b/MAGNUS/g' scripts/test-cli.sh
```

### 8.5 Verify + commit

- [ ] `cargo check --workspace --all-targets` passes.
- [ ] Final allowlist audit:

```bash
rg -in '\btempo\b' \
  --glob '!target/**' --glob '!.git/**' --glob '!Cargo.lock' \
  --glob '!CHANGELOG.md' --glob '!LICENSE*' --glob '!README.md' \
  --glob '!.gitmodules' --glob '!mips/ref-impls/lib/**'
```

Expect ≤5 matches, all intentional (NAMESPACE, keccak domain separators,
behavioral comment).

- [ ] Commit:
  `refactor(phase-8): CODEOWNERS, tempoup->magnusup, NAMESPACE note, fork cutover`.

---

## Task 9 — Verification + PR

### 9.1 Final acceptance gates

```bash
cargo check --workspace --all-features
cargo build --workspace --release
cargo test --workspace --lib --no-fail-fast
cargo fmt --check
cargo doc --workspace --no-deps
```

### 9.2 Binary smoke test

```bash
just magnus-dev-up
sleep 60
curl -s -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":1}' \
  http://localhost:8545
# Expect: {"jsonrpc":"2.0","id":1,"result":"magnus/v1.6.0-<sha>/..."}
curl -s -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545
# Expect a non-zero block number (blocks are being produced)
just magnus-dev-down
```

### 9.3 Push and open PR

```bash
git push -u origin refactor/magnus-rename
gh pr create --title "refactor: rename Tempo to Magnus + workspace restructure" \
             --body-file docs/superpowers/specs/2026-04-18-magnus-rename-design.md
```

---

## Rollback

Everything on `refactor/magnus-rename`. Post-merge regressions revert in
reverse phase order on `main`.

## Notes for executing agents

- Run each phase's `cargo check` **before** committing. The expected-fail gate
  in phase 3 is the only exception and must be called out in the commit body.
- If a pattern over-renames (captured during this plan's execution for Reth
  `tip_*` methods), add an explicit revert as a fixup commit rather than
  retrying the pattern with more lookaheads.
- `perl -i -pe` is preferred over `sed` for regex features (lookaheads,
  backreferences) and cross-platform consistency.
- `rg` is preferred over `grep -r` for speed and gitignore-awareness.
- Do NOT rename `tip_timestamp`, `tip_hash`, `latest_tip`, `tip_block*`,
  `tip_header` — these are upstream Reth chain-HEAD methods.
- Do NOT rename consensus NAMESPACE, RPC `#[rpc(namespace = "tempo")]`,
  Solidity keccak domain separators, or the `magnus-std` submodule URL.
  All behavioral; deferred to chain-ID-change spec.
