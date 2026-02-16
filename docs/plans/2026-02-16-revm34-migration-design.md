# Revm 34 Migration Design

**Date**: 2026-02-16
**Status**: Approved
**Scope**: Upgrade reth dependencies to v1.10.2 and fix revm 34 API breaking changes

## Problem

The workspace declares `revm = "34.0.0"` but the 44 reth crates are pinned to rev `d76babb2` (reth 1.9.x), which pulls in revm 33.1.0. This causes a `revm_context_interface` v13/v14 version conflict that blocks compilation of `magnus-vm` and downstream crates like `magnus-precompile-registry`.

## Approach

Two-phase migration:

1. **Phase 1 — Dependency alignment**: Bump all 44 reth crate git refs from rev `d76babb2` to reth v1.10.2 tag `8e3b5e6a99439561b73c5dd31bd3eced2e994d60`, which uses revm 34.0.0 natively.

2. **Phase 2 — API migration**: Fix ~30 compilation errors across `magnus-vm` caused by revm 34 breaking changes.

## Phase 1: Reth Version Bump

Update `Cargo.toml` workspace dependencies. All 44 reth crates share the same git ref, so this is a single find-and-replace of the rev string.

**Current**: `rev = "d76babb2f17773f79c9cf1eda497c539bd5cf553"`
**Target**: `rev = "8e3b5e6a99439561b73c5dd31bd3eced2e994d60"` (reth v1.10.2)

### Affected Downstream Crates

Six workspace crates directly import reth-evm or reth-revm and may need source changes if reth's public API changed between 1.9.x and 1.10.2:

| Crate | reth deps | Risk |
|-------|-----------|------|
| `magnus-vm` | reth-evm | **High** — already has 30 errors |
| `magnus-evm` | reth-evm, reth-evm-ethereum, reth-revm | Medium |
| `magnus-payload` | reth-evm, reth-revm | Medium |
| `magnus-consensus-engine` | reth-evm, reth-revm | Medium |
| `magnus-mempool` | reth-evm | Low |
| `magnus-provider` | reth-evm (optional) | Low |

## Phase 2: API Migration (magnus-vm)

### Category A: Gas Calculation — `get_tokens_in_calldata` removed (3 locations)

**Files**: handler.rs:25, handler.rs:87, handler.rs:1231

Revm 34 removed `get_tokens_in_calldata`. Replace with inline calldata token counting:

```rust
fn count_calldata_tokens(data: &[u8], is_istanbul: bool) -> u64 {
    let zero_cost: u64 = 4;
    let nonzero_cost: u64 = if is_istanbul { 16 } else { 68 };
    data.iter()
        .map(|&b| if b == 0 { zero_cost } else { nonzero_cost })
        .sum()
}
```

Or use the new revm 34 equivalent if one exists under a different name/module.

### Category B: Transaction Environment — `set_gas_limit()` removed (4 locations)

**Files**: exec.rs:105, exec.rs:124, tx.rs:225-226

`TxEnv::set_gas_limit()` and the `TransactionEnv::set_gas_limit()` trait method were removed. Replace with direct field assignment:

```rust
// Before
tx.set_gas_limit(SYSTEM_CALL_GAS_LIMIT);
// After
tx.gas_limit = SYSTEM_CALL_GAS_LIMIT;
```

For the `TransactionEnv` trait impl in tx.rs, either remove the method or update to match the new trait signature.

### Category C: Account State Mutations (4 locations)

**Files**: handler.rs:358, handler.rs:624, handler.rs:673, handler.rs:738

Methods removed from `JournaledAccount`:
- `bump_nonce()` — replace with `info.nonce += 1` or journal-level nonce increment
- `touch()` — replace with journal touch API or mark account as touched via status flag
- `delegate()` — replace with new delegation API (EIP-7702 related)

These require importing `JournaledAccountTr` trait and using its new method signatures.

### Category D: Validation — `validate_initial_tx_gas` signature change (1 location)

**File**: handler.rs:1139

The function signature changed in revm 34. Update parameters to match the new API (likely added/removed parameters related to EIP-7623 floor cost).

### Category E: Configuration — `disable_fee_charge` removed (1 location)

**File**: handler.rs:1022

`CfgEnv.disable_fee_charge` was removed. The fee-charge-disable logic may have moved to a different mechanism or been replaced by a spec-level flag.

### Category F: Error Handling — `as_invalid_tx_err()` removed (4 locations)

**Files**: error.rs:148-153, handler.rs:1159, handler.rs:2216

The `InvalidTxError::as_invalid_tx_err()` trait method was removed. Replace with direct pattern matching:

```rust
// Before
if let Some(InvalidTransaction::...) = error.as_invalid_tx_err() { ... }
// After
match error {
    MagnusInvalidTransaction::EthInvalidTransaction(inner) => { ... }
    _ => { ... }
}
```

### Category G: Test Updates (~10 locations)

Test code in handler.rs, tx.rs, error.rs, and evm.rs that calls removed APIs. These follow the same patterns as above.

## Success Criteria

1. `cargo check --workspace` passes with zero errors
2. `cargo test -p magnus-vm` passes
3. `cargo test -p magnus-precompile-registry` passes
4. `cargo test --workspace` passes (full suite)

## Risk Mitigation

- **Reth API drift**: Reth 1.10.2 may rename or restructure public types beyond what revm 34 requires. Run `cargo check` after Phase 1 before starting Phase 2 to isolate reth-specific breakage from revm-specific breakage.
- **Transitive dependency conflicts**: After bumping reth, run `cargo tree -d` to check for duplicate crate versions that could cause type mismatches.
- **EIP-7702 delegation changes**: The `delegate()` removal is EIP-7702 related. Check reth 1.10.2's approach to delegation to ensure Magnus's EIP-7702 support remains correct.
