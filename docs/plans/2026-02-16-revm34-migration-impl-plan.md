# Revm 34 Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Resolve the revm 33/34 version conflict by upgrading reth to v1.10.2 and fixing all resulting API breakage across the workspace.

**Architecture:** Two-phase: (1) bump all 44 reth git refs in workspace Cargo.toml, (2) fix compilation errors revealed by `cargo check`, working crate-by-crate from leaf dependencies inward.

**Tech Stack:** Rust, revm 34.0.0, reth v1.10.2, alloy-evm 0.27

---

### Task 1: Bump reth git refs in workspace Cargo.toml

**Files:**
- Modify: `Cargo.toml` (workspace root, lines 166-209)

**Step 1: Replace the reth rev string**

Find-and-replace all occurrences of the old rev with the new one:

```
Old: rev = "d76babb2f17773f79c9cf1eda497c539bd5cf553"
New: rev = "8e3b5e6a99439561b73c5dd31bd3eced2e994d60"
```

There are 44 occurrences to replace. Use `replace_all` or sed to change them all at once.

**Step 2: Run cargo update to fetch new reth**

Run: `cargo update -p reth-evm`
Expected: Cargo fetches reth v1.10.2 git checkout and updates `Cargo.lock`

**Step 3: Check for duplicate dependency versions**

Run: `cargo tree -d 2>&1 | grep -i "revm-context-interface\|alloy-evm" | head -20`
Expected: No duplicate versions of `revm-context-interface` (should all be v14.0.0 now)

If alloy-evm version conflict appears, update `alloy-evm` version in Cargo.toml to match what reth 1.10.2 requires.

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump reth from 1.9.x to v1.10.2 (revm 34 alignment)"
```

---

### Task 2: Triage — run cargo check and categorize errors

**Step 1: Run cargo check on the workspace**

Run: `cargo check --workspace 2>&1 | tee /tmp/revm34-errors.txt`

This will produce the actual error list after the reth bump. Many errors from the design doc may have resolved (e.g., `bump_nonce`, `touch`, `delegate` which still exist in revm 34 but failed due to version conflict).

**Step 2: Categorize remaining errors**

Group errors by file and type. Create a mental checklist. The expected categories are:

1. **`get_tokens_in_calldata` signature change** — second param changed from `bool` to `u64`
   - Files: `handler.rs:87`, `handler.rs:1231` (import at line 25)
   - Fix: `get_tokens_in_calldata(&data, true)` → `get_tokens_in_calldata(&data, NON_ZERO_BYTE_MULTIPLIER_ISTANBUL)` or use `get_tokens_in_calldata_istanbul(&data)`

2. **`TxEnv::set_gas_limit()` removed** — no longer a method on TxEnv
   - Files: `exec.rs:105`, `exec.rs:124`
   - Fix: `tx.set_gas_limit(X)` → `tx.gas_limit = X`

3. **`TransactionEnv` trait changes** — methods like `set_gas_limit`, `set_nonce`, `set_access_list` may have changed
   - Files: `tx.rs:224-240`
   - Fix: Update trait impl to match new trait definition from reth 1.10.2

4. **`cfg.disable_fee_charge` field access** — may need to use `cfg.is_fee_charge_disabled()` method instead
   - Files: `handler.rs:1022`

5. **`as_invalid_tx_err()` on EVMError** — may have changed in alloy-evm
   - Files: `error.rs:148`, `handler.rs:1159`, `handler.rs:2216`, `error.rs:273,277`

6. **Reth API changes** — new types, renamed methods, changed trait bounds in reth 1.10.2
   - Files: potentially any crate that imports reth types

**Step 3: Note any unexpected errors**

If `cargo check` reveals errors in crates beyond `magnus-vm`, note them. Likely candidates: `magnus-evm`, `magnus-payload`, `magnus-consensus-engine`.

Do NOT commit anything in this task — this is purely diagnostic.

---

### Task 3: Fix `get_tokens_in_calldata` signature change

**Files:**
- Modify: `crates/execution/vm/src/handler.rs`

**Context:** In revm 34 (revm-context-interface 14.0.0), `get_tokens_in_calldata` changed from `(data: &[u8], is_istanbul: bool) -> u64` to `(data: &[u8], non_zero_data_multiplier: u64) -> u64`. A convenience function `get_tokens_in_calldata_istanbul(data)` was also added.

**Step 1: Update import (line 25)**

If the function moved modules, update the import path. It may now be at:
- `revm::context_interface::cfg::gas::get_tokens_in_calldata`
- Or still re-exported through `revm::interpreter::gas`

If `get_tokens_in_calldata_istanbul` is available in the import, add it.

**Step 2: Fix call at line 87 (WebAuthn signature gas)**

```rust
// Before:
let tokens = get_tokens_in_calldata(&webauthn_sig.webauthn_data, true);
// After (option A — use istanbul helper):
let tokens = get_tokens_in_calldata_istanbul(&webauthn_sig.webauthn_data);
// After (option B — use multiplier constant):
let tokens = get_tokens_in_calldata(&webauthn_sig.webauthn_data, NON_ZERO_BYTE_MULTIPLIER_ISTANBUL);
```

**Step 3: Fix call at line 1231 (AA batch intrinsic gas)**

Same pattern as Step 2.

**Step 4: Verify**

Run: `cargo check -p magnus-vm 2>&1 | grep -c "get_tokens_in_calldata"`
Expected: 0 (no more errors for this function)

**Step 5: Commit**

```bash
git add crates/execution/vm/src/handler.rs
git commit -m "fix: update get_tokens_in_calldata to revm 34 signature"
```

---

### Task 4: Fix `TxEnv::set_gas_limit` removal

**Files:**
- Modify: `crates/execution/vm/src/exec.rs`
- Modify: `crates/execution/vm/src/tx.rs`

**Step 1: Fix exec.rs line 105 (system_call_one_with_caller)**

```rust
// Before:
tx.set_gas_limit(SYSTEM_CALL_GAS_LIMIT);
// After:
tx.gas_limit = SYSTEM_CALL_GAS_LIMIT;
```

**Step 2: Fix exec.rs line 124 (inspect_one_system_call_with_caller)**

Same pattern as Step 1.

**Step 3: Fix TransactionEnv impl in tx.rs (lines 224-240)**

Check the new `TransactionEnv` trait definition from reth 1.10.2. If `set_gas_limit` was removed from the trait, remove the impl. If it was renamed or changed, update accordingly.

If the trait still requires `set_gas_limit`:
```rust
fn set_gas_limit(&mut self, gas_limit: u64) {
    self.inner.gas_limit = gas_limit;
}
```

If `set_nonce` was also changed:
```rust
fn set_nonce(&mut self, nonce: u64) {
    self.inner.nonce = nonce;
}
```

If `set_access_list` was also changed:
```rust
fn set_access_list(&mut self, access_list: AccessList) {
    self.inner.access_list = access_list;
}
```

**Step 4: Fix tests in tx.rs**

Update test code that calls `tx_env.set_gas_limit(...)` to either use the new API or direct field assignment.

**Step 5: Verify**

Run: `cargo check -p magnus-vm 2>&1 | grep -c "set_gas_limit\|set_nonce\|set_access_list"`
Expected: 0

**Step 6: Commit**

```bash
git add crates/execution/vm/src/exec.rs crates/execution/vm/src/tx.rs
git commit -m "fix: replace TxEnv setter methods with direct field access for revm 34"
```

---

### Task 5: Fix `cfg.disable_fee_charge` field access

**Files:**
- Modify: `crates/execution/vm/src/handler.rs`

**Step 1: Fix line 1022**

Check whether `disable_fee_charge` is still a field on `CfgEnv` in revm 34. If it was replaced by a method on the `Cfg` trait:

```rust
// Before:
if context.cfg.disable_fee_charge
// After:
if context.cfg.is_fee_charge_disabled()
```

Note: Other similar accessors in the file already use the method form (`is_nonce_check_disabled()`, `is_base_fee_check_disabled()`), so this may just be a consistency fix.

**Step 2: Verify**

Run: `cargo check -p magnus-vm 2>&1 | grep "disable_fee_charge"`
Expected: 0

**Step 3: Commit**

```bash
git add crates/execution/vm/src/handler.rs
git commit -m "fix: use is_fee_charge_disabled() method instead of field access"
```

---

### Task 6: Fix `as_invalid_tx_err` if removed from traits

**Files:**
- Modify: `crates/execution/vm/src/error.rs`
- Modify: `crates/execution/vm/src/handler.rs`

**Context:** The `InvalidTxError` trait from `alloy_evm::error` defines `as_invalid_tx_err()`. Check if it still exists in the alloy-evm version used by reth 1.10.2. If it does, this task is a no-op. If it was removed:

**Step 1: Fix error.rs InvalidTxError impl (lines 140-154)**

If `as_invalid_tx_err` was removed from the trait, remove the method from the impl block. If a new required method was added, implement it.

**Step 2: Fix handler.rs line 1159 (catch_error)**

Replace `error.as_invalid_tx_err()` with direct pattern matching:

```rust
// Before:
) = error.as_invalid_tx_err()
// After — depends on EVMError structure:
if evm.ctx.tx.is_subblock_transaction() {
    if let EVMError::Transaction(
        MagnusInvalidTransaction::CollectFeePreTx(_)
        | MagnusInvalidTransaction::EthInvalidTransaction(
            InvalidTransaction::LackOfFundForMaxFee { .. },
        ),
    ) = &error {
```

**Step 3: Fix handler.rs test at line 2216**

Same pattern — replace `err.as_invalid_tx_err()` with direct matching.

**Step 4: Fix error.rs tests (lines 273, 277)**

```rust
// Before:
assert!(err.as_invalid_tx_err().is_some());
// After (if method removed):
assert!(matches!(err, MagnusInvalidTransaction::EthInvalidTransaction(_)));
```

**Step 5: Verify**

Run: `cargo check -p magnus-vm 2>&1 | grep "as_invalid_tx_err"`
Expected: 0

**Step 6: Commit**

```bash
git add crates/execution/vm/src/error.rs crates/execution/vm/src/handler.rs
git commit -m "fix: remove as_invalid_tx_err usage, use pattern matching"
```

---

### Task 7: Fix remaining account mutation methods if needed

**Files:**
- Modify: `crates/execution/vm/src/handler.rs` (lines 358, 624, 673, 738)

**Context:** `bump_nonce()`, `touch()`, and `delegate()` still exist on `JournaledAccountTr` in revm 34 (revm-context-interface 14.0.0). After the reth bump resolves the version conflict, these should compile. However, `bump_nonce()` now returns `bool` — if any call site ignores the return value without `let _ =`, the compiler may warn.

**Step 1: Check if these methods compile after reth bump**

Run: `cargo check -p magnus-vm 2>&1 | grep -i "bump_nonce\|\.touch()\|\.delegate("`

If no errors: skip this task entirely.

**Step 2: If `bump_nonce` return value causes issues**

Add `let _ =` or handle the `bool` return:

```rust
// Line 358:
let _ = caller_acc.data.bump_nonce();
// Line 738:
let _ = caller_account.bump_nonce();
```

**Step 3: If `touch()` or `delegate()` need trait import**

Add the import:
```rust
use revm::context_interface::journaled_state::account::JournaledAccountTr;
```

**Step 4: Verify and commit if changes were made**

```bash
git add crates/execution/vm/src/handler.rs
git commit -m "fix: handle bump_nonce bool return, import JournaledAccountTr"
```

---

### Task 8: Fix `validate_initial_tx_gas` signature if changed

**Files:**
- Modify: `crates/execution/vm/src/handler.rs` (line 1139)

**Context:** The current code already passes `(tx, spec, evm.ctx.cfg.is_eip7623_disabled())`. Check if this matches the revm 34 signature. The function in revm-handler 15.0.0 takes `(tx: impl Transaction, spec: SpecId, is_eip7623_disabled: bool)`.

**Step 1: Check if it compiles**

Run: `cargo check -p magnus-vm 2>&1 | grep "validate_initial_tx_gas"`

If no errors: skip this task.

**Step 2: If the `spec` conversion changed**

The current code does `evm.ctx_ref().cfg().spec().into()` to convert `MagnusHardfork` to `SpecId`. If the `.into()` conversion no longer works, update accordingly.

**Step 3: Commit if changes were made**

```bash
git add crates/execution/vm/src/handler.rs
git commit -m "fix: update validate_initial_tx_gas call to match revm 34"
```

---

### Task 9: Fix errors in downstream crates (magnus-evm, magnus-payload, etc.)

**Files:**
- Potentially: `crates/execution/evm/src/`
- Potentially: `crates/node/payload/src/`
- Potentially: `crates/consensus/consensus-engine/src/`
- Potentially: `crates/node/mempool/src/`
- Potentially: `crates/sdk/provider/src/`

**Step 1: Run workspace check**

Run: `cargo check --workspace 2>&1 | grep "^error" | sort -u`

Group errors by crate. For each affected crate:

**Step 2: Fix each crate**

The fixes will likely follow the same patterns as Tasks 3-8:
- Setter methods → direct field access
- Trait method changes → update impls
- Type renames → update imports

Work through each crate alphabetically.

**Step 3: Verify all crates compile**

Run: `cargo check --workspace`
Expected: No errors

**Step 4: Commit per crate**

```bash
git commit -m "fix(magnus-evm): adapt to reth 1.10.2 API changes"
git commit -m "fix(magnus-payload): adapt to reth 1.10.2 API changes"
# etc.
```

---

### Task 10: Run tests

**Step 1: Run magnus-vm tests**

Run: `cargo test -p magnus-vm`
Expected: All tests pass

**Step 2: Run magnus-precompile-registry tests**

Run: `cargo test -p magnus-precompile-registry`
Expected: All tests pass (these were previously blocked by the version conflict)

**Step 3: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 4: Fix any test failures**

If tests fail due to API changes (e.g., `set_gas_limit` in test code), apply the same patterns from Tasks 3-8.

**Step 5: Final commit**

```bash
git add -A
git commit -m "test: fix test code for revm 34 API changes"
```

---

### Task 11: Final verification

**Step 1: Clean build**

Run: `cargo clean && cargo check --workspace`
Expected: Clean compilation with no warnings from magnus-vm

**Step 2: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 3: Check for duplicate deps**

Run: `cargo tree -d 2>&1 | grep "revm-context-interface"`
Expected: Only v14.0.0 (no v13 duplicates)
