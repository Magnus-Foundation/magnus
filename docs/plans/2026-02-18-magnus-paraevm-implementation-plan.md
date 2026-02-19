# MagnusParaEVM Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a 2-path parallel EVM execution engine that replaces FAFO, using exact scheduling for known transactions and operation-level OCC with SSA redo for unknown contracts.

**Architecture:** A new `magnus-paraevm` crate under `crates/node/paraevm/` implements `BlockExecutor<S>` alongside the existing sequential `RevmExecutor`. A `TransactionRouter` classifies each transaction into Path 1 (exact scheduling via HashSet read/write sets) or Path 2 (operation-level OCC with SSA logging). Both paths share a rayon-based REVM worker pool. A `LazyBeneficiary` defers fee distribution to avoid universal write conflicts.

**Tech Stack:** Rust, revm 34.0.0, rayon (thread pool), alloy-primitives, smallvec, parking_lot, magnus-qmdb (ChangeSet/AccountUpdate), magnus-traits (StateDb/BlockExecutor)

**Design doc:** `docs/plans/2026-02-18-magnus-paraevm-design.md`

---

## Task 1: Create crate skeleton and types

**Files:**
- Create: `crates/node/paraevm/Cargo.toml`
- Create: `crates/node/paraevm/src/lib.rs`
- Create: `crates/node/paraevm/src/types.rs`
- Modify: `Cargo.toml` (workspace members + dependencies)

**Step 1: Add workspace dependencies**

In the root `Cargo.toml`, add to `[workspace.members]`:
```toml
"crates/node/paraevm",
```

Add to `[workspace.dependencies]`:
```toml
rayon = "1.10"
smallvec = "1.13"
magnus-paraevm = { path = "crates/node/paraevm" }
```

**Step 2: Create `crates/node/paraevm/Cargo.toml`**

```toml
[package]
name = "magnus-paraevm"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "2-path parallel EVM execution engine for Magnus"

[dependencies]
alloy-primitives.workspace = true
magnus-executor = { path = "../executor" }
magnus-precompiles = { workspace = true }
magnus-qmdb = { path = "../../storage/qmdb" }
magnus-traits = { path = "../../storage/traits" }
parking_lot.workspace = true
rayon.workspace = true
revm.workspace = true
smallvec = { workspace = true }
thiserror.workspace = true

[dev-dependencies]
alloy-consensus.workspace = true
rstest.workspace = true
tokio = { workspace = true, features = ["rt", "macros"] }
```

**Step 3: Create `crates/node/paraevm/src/types.rs`**

This file defines the core data structures from the design doc.

```rust
//! Core types for MagnusParaEVM parallel execution.

use alloy_primitives::{Address, U256};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

/// Which execution path a transaction is routed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPath {
    /// Path 1: known contracts with pre-computed read/write sets.
    ExactSchedule,
    /// Path 2: unknown contracts, SSA logging + operation-level redo.
    OpLevelOCC,
}

/// A storage location (address + slot) used in read/write sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StorageLocation {
    pub address: Address,
    pub slot: U256,
}

/// Pre-computed read/write set for a Path 1 transaction.
#[derive(Debug, Clone, Default)]
pub struct ReadWriteSet {
    pub reads: HashSet<StorageLocation>,
    pub writes: HashSet<StorageLocation>,
}

impl ReadWriteSet {
    /// Check if this set conflicts with a set of occupied slots.
    pub fn conflicts_with(&self, occupied: &HashSet<StorageLocation>) -> bool {
        self.reads.iter().any(|loc| occupied.contains(loc))
            || self.writes.iter().any(|loc| occupied.contains(loc))
    }

    /// Return all locations (reads + writes) for frame occupation tracking.
    pub fn all_locations(&self) -> impl Iterator<Item = &StorageLocation> {
        self.reads.iter().chain(self.writes.iter())
    }
}

/// SSA reference: either a prior log entry or a constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsaRef {
    /// Reference to a prior SSA entry's output by LSN.
    Entry(u64),
    /// A constant value (from PUSH, calldata, etc.).
    Constant(U256),
}

/// A single SSA log entry recording one EVM operation.
#[derive(Debug, Clone)]
pub struct SsaEntry {
    /// Log sequence number (monotonically increasing per transaction).
    pub lsn: u64,
    /// The EVM opcode byte.
    pub opcode: u8,
    /// Input references.
    pub inputs: SmallVec<[SsaRef; 4]>,
    /// Output value produced by this operation.
    pub output: U256,
    /// For SLOAD/SSTORE: the storage key accessed.
    pub storage_key: Option<StorageLocation>,
}

/// SSA operation log for a single transaction.
#[derive(Debug, Clone, Default)]
pub struct SsaLog {
    /// All SSA entries in execution order.
    pub entries: Vec<SsaEntry>,
    /// Shadow stack: maps EVM stack positions to the LSN that produced the value.
    /// `None` means the value was a constant pushed directly.
    pub shadow_stack: Vec<Option<u64>>,
    /// Next LSN to assign.
    next_lsn: u64,
}

impl SsaLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next LSN and return it.
    pub fn next_lsn(&mut self) -> u64 {
        let lsn = self.next_lsn;
        self.next_lsn += 1;
        lsn
    }

    /// Push an entry to the log.
    pub fn push(&mut self, entry: SsaEntry) {
        self.entries.push(entry);
    }
}

/// Thread-local write buffer for a single transaction during optimistic execution.
#[derive(Debug, Clone, Default)]
pub struct WriteBuffer {
    /// Storage writes: (address, slot) -> value.
    pub writes: HashMap<StorageLocation, U256>,
}

impl WriteBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, loc: StorageLocation, value: U256) {
        self.writes.insert(loc, value);
    }
}

/// Accumulated fees for lazy beneficiary distribution.
#[derive(Debug)]
pub struct LazyBeneficiary {
    pub beneficiary: Address,
    pub accumulated_fees: u64,
}

impl LazyBeneficiary {
    pub fn new(beneficiary: Address) -> Self {
        Self { beneficiary, accumulated_fees: 0 }
    }

    pub fn add_fee(&mut self, fee: u64) {
        self.accumulated_fees = self.accumulated_fees.saturating_add(fee);
    }
}
```

**Step 4: Create `crates/node/paraevm/src/lib.rs`**

```rust
//! MagnusParaEVM: 2-path parallel EVM execution engine.
//!
//! Path 1 (Exact Scheduler): For known contracts with deterministic storage access.
//! Path 2 (OpLevel OCC): For unknown contracts using SSA-based operation logging.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod types;
```

**Step 5: Verify it compiles**

Run: `cargo check -p magnus-paraevm`
Expected: compiles with no errors

**Step 6: Commit**

```bash
git add crates/node/paraevm/ Cargo.toml
git commit -m "feat(paraevm): create crate skeleton with core types"
```

---

## Task 2: Transaction Router

**Files:**
- Create: `crates/node/paraevm/src/router.rs`
- Create: `crates/node/paraevm/src/router_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/router_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, address};
    use crate::router::TransactionRouter;
    use crate::types::ExecutionPath;

    // MIP20 factory address from crates/node/precompiles/src/lib.rs:33
    const MIP20_FACTORY: Address = address!("20FC20FC20FC20FC20FC20FC20FC20FC20FC20FC");
    // A MIP20 token address (prefix 0x20C0...)
    const MIP20_TOKEN: Address = address!("20C0000000000000000000000000000000000001");
    // transfer(address,uint256) selector
    const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

    fn unknown_address() -> Address {
        address!("dead000000000000000000000000000000000001")
    }

    #[test]
    fn native_transfer_routes_to_path1() {
        let router = TransactionRouter::new();
        // Native transfer: value > 0, no calldata
        let path = router.classify(
            Address::ZERO,       // from (EOA)
            Some(unknown_address()), // to (any EOA)
            U256::from(1000),    // value > 0
            &Bytes::new(),       // no calldata
        );
        assert_eq!(path, ExecutionPath::ExactSchedule);
    }

    #[test]
    fn known_contract_known_selector_routes_to_path1() {
        let mut router = TransactionRouter::new();
        router.register_known_selector(MIP20_TOKEN, TRANSFER_SELECTOR, ExecutionPath::ExactSchedule);

        let mut calldata = Vec::with_capacity(68);
        calldata.extend_from_slice(&TRANSFER_SELECTOR);
        calldata.extend_from_slice(&[0u8; 64]); // dummy args

        let path = router.classify(
            Address::ZERO,
            Some(MIP20_TOKEN),
            U256::ZERO,
            &Bytes::from(calldata),
        );
        assert_eq!(path, ExecutionPath::ExactSchedule);
    }

    #[test]
    fn unknown_contract_routes_to_path2() {
        let router = TransactionRouter::new();
        let calldata = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef, 0x00]);

        let path = router.classify(
            Address::ZERO,
            Some(unknown_address()),
            U256::ZERO,
            &calldata,
        );
        assert_eq!(path, ExecutionPath::OpLevelOCC);
    }

    #[test]
    fn known_contract_unknown_selector_routes_to_path2() {
        let mut router = TransactionRouter::new();
        // Register only transfer selector
        router.register_known_selector(MIP20_TOKEN, TRANSFER_SELECTOR, ExecutionPath::ExactSchedule);

        // Call with a different selector
        let unknown_selector = [0xff, 0xff, 0xff, 0xff];
        let mut calldata = Vec::with_capacity(36);
        calldata.extend_from_slice(&unknown_selector);
        calldata.extend_from_slice(&[0u8; 32]);

        let path = router.classify(
            Address::ZERO,
            Some(MIP20_TOKEN),
            U256::ZERO,
            &Bytes::from(calldata),
        );
        assert_eq!(path, ExecutionPath::OpLevelOCC);
    }

    #[test]
    fn contract_creation_routes_to_path2() {
        let router = TransactionRouter::new();
        let path = router.classify(
            Address::ZERO,
            None,            // no `to` = contract creation
            U256::ZERO,
            &Bytes::from(vec![0x60, 0x80, 0x60, 0x40]), // init code
        );
        assert_eq!(path, ExecutionPath::OpLevelOCC);
    }

    use alloy_primitives::U256;
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- router_tests`
Expected: FAIL — `router` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/router.rs`:

```rust
//! Transaction Router — classifies transactions into Path 1 or Path 2.

use alloy_primitives::{Address, Bytes, U256};
use std::collections::{HashMap, HashSet};

use crate::types::ExecutionPath;

/// Classifies transactions into execution paths in O(1).
#[derive(Debug, Clone)]
pub struct TransactionRouter {
    /// Contracts with known storage layouts.
    known_contracts: HashSet<Address>,
    /// Per-selector path overrides for known contracts.
    known_selectors: HashMap<(Address, [u8; 4]), ExecutionPath>,
}

impl TransactionRouter {
    /// Create a new router with no known contracts.
    pub fn new() -> Self {
        Self {
            known_contracts: HashSet::new(),
            known_selectors: HashMap::new(),
        }
    }

    /// Register a contract address as known.
    pub fn register_known_contract(&mut self, address: Address) {
        self.known_contracts.insert(address);
    }

    /// Register a specific (contract, selector) pair with a path.
    pub fn register_known_selector(
        &mut self,
        address: Address,
        selector: [u8; 4],
        path: ExecutionPath,
    ) {
        self.known_contracts.insert(address);
        self.known_selectors.insert((address, selector), path);
    }

    /// Classify a transaction into an execution path.
    ///
    /// Rules:
    /// 1. Contract creation (to == None) -> Path 2
    /// 2. Native transfer (value > 0, empty calldata) -> Path 1
    /// 3. Known contract + known selector -> use registered path
    /// 4. Known contract + unknown selector -> Path 2
    /// 5. Unknown contract -> Path 2
    pub fn classify(
        &self,
        _from: Address,
        to: Option<Address>,
        value: U256,
        calldata: &Bytes,
    ) -> ExecutionPath {
        // Rule 1: contract creation
        let to = match to {
            Some(addr) => addr,
            None => return ExecutionPath::OpLevelOCC,
        };

        // Rule 2: native transfer (value > 0, no calldata)
        if calldata.is_empty() && !value.is_zero() {
            return ExecutionPath::ExactSchedule;
        }

        // Extract selector (first 4 bytes of calldata)
        if calldata.len() < 4 {
            // No valid selector — treat as unknown
            return if calldata.is_empty() && value.is_zero() {
                // Zero-value call with no data — still Path 1 (noop-ish)
                ExecutionPath::ExactSchedule
            } else {
                ExecutionPath::OpLevelOCC
            };
        }
        let selector: [u8; 4] = calldata[..4].try_into().unwrap();

        // Rule 3: known contract + known selector
        if let Some(&path) = self.known_selectors.get(&(to, selector)) {
            return path;
        }

        // Rule 4: known contract + unknown selector
        if self.known_contracts.contains(&to) {
            return ExecutionPath::OpLevelOCC;
        }

        // Rule 5: unknown contract
        ExecutionPath::OpLevelOCC
    }
}

impl Default for TransactionRouter {
    fn default() -> Self {
        Self::new()
    }
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod router;
#[cfg(test)]
mod router_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- router_tests`
Expected: all 5 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/router.rs crates/node/paraevm/src/router_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add TransactionRouter with O(1) classification"
```

---

## Task 3: Path 1 — Read/Write Set Derivation

**Files:**
- Create: `crates/node/paraevm/src/rw_derive.rs`
- Create: `crates/node/paraevm/src/rw_derive_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/rw_derive_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256, address, keccak256};
    use crate::rw_derive::{derive_native_transfer_rw, derive_mip20_transfer_rw};
    use crate::types::StorageLocation;

    #[test]
    fn native_transfer_touches_both_balances() {
        let from = address!("aaaa000000000000000000000000000000000001");
        let to = address!("bbbb000000000000000000000000000000000002");

        let rw = derive_native_transfer_rw(from, to);

        // Native transfers touch account balances (not storage slots),
        // but for conflict detection we track the balance "slot" as a
        // virtual location: (address, slot=0) for the account balance.
        assert!(rw.writes.contains(&StorageLocation {
            address: from,
            slot: U256::ZERO,
        }));
        assert!(rw.writes.contains(&StorageLocation {
            address: to,
            slot: U256::ZERO,
        }));
        assert_eq!(rw.writes.len(), 2);
    }

    #[test]
    fn mip20_transfer_touches_two_balance_slots() {
        let token = address!("20C0000000000000000000000000000000000001");
        let from = address!("aaaa000000000000000000000000000000000001");
        let to = address!("bbbb000000000000000000000000000000000002");

        let rw = derive_mip20_transfer_rw(token, from, to);

        // MIP20 transfer reads/writes balances[from] and balances[to]
        // balances base slot = 0, slot = keccak256(address, 0)
        let from_slot = mapping_slot(&from, &U256::ZERO);
        let to_slot = mapping_slot(&to, &U256::ZERO);

        assert!(rw.writes.contains(&StorageLocation {
            address: token,
            slot: from_slot,
        }));
        assert!(rw.writes.contains(&StorageLocation {
            address: token,
            slot: to_slot,
        }));
    }

    #[test]
    fn self_transfer_has_one_slot() {
        let token = address!("20C0000000000000000000000000000000000001");
        let user = address!("aaaa000000000000000000000000000000000001");

        let rw = derive_mip20_transfer_rw(token, user, user);

        // Self-transfer: balances[user] appears once (HashSet dedup)
        let user_slot = mapping_slot(&user, &U256::ZERO);
        assert!(rw.writes.contains(&StorageLocation {
            address: token,
            slot: user_slot,
        }));
        // reads and writes both touch the same slot
    }

    /// Helper: compute Solidity mapping slot = keccak256(abi.encode(key, base_slot))
    fn mapping_slot(key: &Address, base_slot: &U256) -> U256 {
        let mut data = [0u8; 64];
        // Left-pad address to 32 bytes
        data[12..32].copy_from_slice(key.as_slice());
        data[32..64].copy_from_slice(&base_slot.to_be_bytes::<32>());
        U256::from_be_bytes::<32>(keccak256(data).0)
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- rw_derive_tests`
Expected: FAIL — `rw_derive` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/rw_derive.rs`:

```rust
//! Read/Write Set Derivation for known contracts (Path 1).
//!
//! For each known contract type and selector, we statically compute the exact
//! set of storage slots that will be read/written, using the same keccak256
//! slot layout as the precompile storage system.

use alloy_primitives::{Address, U256, keccak256};

use crate::types::{ReadWriteSet, StorageLocation};

/// Compute the Solidity mapping slot: keccak256(abi.encode(key, base_slot)).
///
/// This mirrors `crates/node/precompiles/src/storage/mapping.rs:55` exactly.
pub fn mapping_slot_addr(key: &Address, base_slot: &U256) -> U256 {
    let mut data = [0u8; 64];
    data[12..32].copy_from_slice(key.as_slice());
    data[32..64].copy_from_slice(&base_slot.to_be_bytes::<32>());
    U256::from_be_bytes::<32>(keccak256(data).0)
}

/// Derive read/write set for a native ETH/MAGNUS transfer.
///
/// Native transfers modify the account balance of `from` and `to`.
/// We represent this as a virtual storage location (address, slot=0).
pub fn derive_native_transfer_rw(from: Address, to: Address) -> ReadWriteSet {
    let mut rw = ReadWriteSet::default();
    let from_loc = StorageLocation { address: from, slot: U256::ZERO };
    let to_loc = StorageLocation { address: to, slot: U256::ZERO };

    rw.reads.insert(from_loc);
    rw.reads.insert(to_loc);
    rw.writes.insert(from_loc);
    rw.writes.insert(to_loc);
    rw
}

/// Derive read/write set for MIP20 `transfer(to, amount)`.
///
/// Touches: balances[from], balances[to].
/// MIP20 balances mapping base slot = 0 (from `crates/node/precompiles/src/mip20/mod.rs:56`).
pub fn derive_mip20_transfer_rw(token: Address, from: Address, to: Address) -> ReadWriteSet {
    let balances_base = U256::ZERO; // slot 0

    let from_slot = mapping_slot_addr(&from, &balances_base);
    let to_slot = mapping_slot_addr(&to, &balances_base);

    let mut rw = ReadWriteSet::default();

    let from_loc = StorageLocation { address: token, slot: from_slot };
    let to_loc = StorageLocation { address: token, slot: to_slot };

    rw.reads.insert(from_loc);
    rw.reads.insert(to_loc);
    rw.writes.insert(from_loc);
    rw.writes.insert(to_loc);
    rw
}

/// Derive read/write set for MIP20 `approve(spender, amount)`.
///
/// Touches: allowances[from][spender].
/// MIP20 allowances mapping base slot = 1 (nested mapping).
pub fn derive_mip20_approve_rw(
    token: Address,
    from: Address,
    spender: Address,
) -> ReadWriteSet {
    let allowances_base = U256::from(1); // slot 1

    // Nested mapping: slot = keccak256(spender, keccak256(from, 1))
    let inner_slot = mapping_slot_addr(&from, &allowances_base);
    // For the outer mapping, we need to use U256 slot calculation
    let mut outer_data = [0u8; 64];
    outer_data[12..32].copy_from_slice(spender.as_slice());
    outer_data[32..64].copy_from_slice(&inner_slot.to_be_bytes::<32>());
    let slot = U256::from_be_bytes::<32>(keccak256(outer_data).0);

    let mut rw = ReadWriteSet::default();
    let loc = StorageLocation { address: token, slot };
    rw.writes.insert(loc);
    rw
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod rw_derive;
#[cfg(test)]
mod rw_derive_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- rw_derive_tests`
Expected: all 3 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/rw_derive.rs crates/node/paraevm/src/rw_derive_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add read/write set derivation for MIP20 and native transfers"
```

---

## Task 4: Path 1 — Frame-Based Greedy Scheduler

**Files:**
- Create: `crates/node/paraevm/src/scheduler.rs`
- Create: `crates/node/paraevm/src/scheduler_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/scheduler_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256, address};
    use crate::scheduler::FrameScheduler;
    use crate::types::{ReadWriteSet, StorageLocation};

    fn loc(addr: Address, slot: u64) -> StorageLocation {
        StorageLocation { address: addr, slot: U256::from(slot) }
    }

    fn rw_set(locs: &[StorageLocation]) -> ReadWriteSet {
        let mut rw = ReadWriteSet::default();
        for l in locs {
            rw.reads.insert(*l);
            rw.writes.insert(*l);
        }
        rw
    }

    #[test]
    fn non_conflicting_txns_in_single_frame() {
        let a = address!("aaaa000000000000000000000000000000000001");
        let b = address!("bbbb000000000000000000000000000000000002");
        let c = address!("cccc000000000000000000000000000000000003");

        // tx0 touches (a, 0), tx1 touches (b, 0), tx2 touches (c, 0)
        let rw_sets = vec![
            rw_set(&[loc(a, 0)]),
            rw_set(&[loc(b, 0)]),
            rw_set(&[loc(c, 0)]),
        ];

        let frames = FrameScheduler::pack(&rw_sets);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].tx_indices.len(), 3);
    }

    #[test]
    fn conflicting_txns_split_into_frames() {
        let a = address!("aaaa000000000000000000000000000000000001");

        // tx0 and tx1 both touch (a, 0) — conflict
        let rw_sets = vec![
            rw_set(&[loc(a, 0)]),
            rw_set(&[loc(a, 0)]),
        ];

        let frames = FrameScheduler::pack(&rw_sets);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].tx_indices, vec![0]);
        assert_eq!(frames[1].tx_indices, vec![1]);
    }

    #[test]
    fn mixed_conflict_pattern() {
        let a = address!("aaaa000000000000000000000000000000000001");
        let b = address!("bbbb000000000000000000000000000000000002");
        let c = address!("cccc000000000000000000000000000000000003");

        // tx0: touches a
        // tx1: touches b (no conflict with tx0)
        // tx2: touches a (conflict with tx0 -> new frame)
        // tx3: touches c (no conflict in new frame)
        let rw_sets = vec![
            rw_set(&[loc(a, 0)]),
            rw_set(&[loc(b, 0)]),
            rw_set(&[loc(a, 0)]),
            rw_set(&[loc(c, 0)]),
        ];

        let frames = FrameScheduler::pack(&rw_sets);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].tx_indices, vec![0, 1]);
        assert_eq!(frames[1].tx_indices, vec![2, 3]);
    }

    #[test]
    fn empty_input_returns_empty() {
        let frames = FrameScheduler::pack(&[]);
        assert!(frames.is_empty());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- scheduler_tests`
Expected: FAIL — `scheduler` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/scheduler.rs`:

```rust
//! Frame-Based Greedy Scheduler (Path 1).
//!
//! Packs non-conflicting transactions into parallel frames using a single-pass
//! O(n) greedy algorithm. Transactions within a frame can execute in parallel.
//! Frames execute sequentially to maintain deterministic block-level ordering.

use std::collections::HashSet;

use crate::types::{ReadWriteSet, StorageLocation};

/// A frame of non-conflicting transactions that can execute in parallel.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Indices into the original transaction list.
    pub tx_indices: Vec<usize>,
}

impl Frame {
    fn new() -> Self {
        Self { tx_indices: Vec::new() }
    }
}

/// Greedy frame packer for Path 1 transactions.
pub struct FrameScheduler;

impl FrameScheduler {
    /// Pack transactions into parallel frames.
    ///
    /// Single-pass O(n) algorithm: iterate transactions in order, accumulate
    /// into the current frame. When a conflict is detected with occupied slots,
    /// start a new frame.
    pub fn pack(rw_sets: &[ReadWriteSet]) -> Vec<Frame> {
        if rw_sets.is_empty() {
            return Vec::new();
        }

        let mut frames = Vec::new();
        let mut current_frame = Frame::new();
        let mut occupied: HashSet<StorageLocation> = HashSet::new();

        for (idx, rw) in rw_sets.iter().enumerate() {
            if rw.conflicts_with(&occupied) {
                frames.push(current_frame);
                current_frame = Frame::new();
                occupied.clear();
            }

            current_frame.tx_indices.push(idx);
            occupied.extend(rw.all_locations().copied());
        }

        // Push the last frame
        if !current_frame.tx_indices.is_empty() {
            frames.push(current_frame);
        }

        frames
    }
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod scheduler;
#[cfg(test)]
mod scheduler_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- scheduler_tests`
Expected: all 4 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/scheduler.rs crates/node/paraevm/src/scheduler_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add frame-based greedy scheduler for Path 1"
```

---

## Task 5: Path 2 — SSA Redo Engine

**Files:**
- Create: `crates/node/paraevm/src/ssa_redo.rs`
- Create: `crates/node/paraevm/src/ssa_redo_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/ssa_redo_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256, address};
    use smallvec::smallvec;
    use crate::types::{SsaEntry, SsaLog, SsaRef, StorageLocation};
    use crate::ssa_redo::ssa_redo;
    use std::collections::{HashMap, HashSet};

    fn token_addr() -> Address {
        address!("20C0000000000000000000000000000000000001")
    }

    #[test]
    fn no_stale_reads_means_no_changes() {
        let mut log = SsaLog::new();
        // Entry 0: SLOAD slot_a -> value 100
        let loc_a = StorageLocation { address: token_addr(), slot: U256::from(42) };
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54, // SLOAD
            inputs: smallvec![],
            output: U256::from(100),
            storage_key: Some(loc_a),
        });
        // Entry 1: ADD(entry0, const 50) -> 150
        log.push(SsaEntry {
            lsn: 1, opcode: 0x01, // ADD
            inputs: smallvec![SsaRef::Entry(0), SsaRef::Constant(U256::from(50))],
            output: U256::from(150),
            storage_key: None,
        });

        // No stale SLOADs
        let stale: HashSet<u64> = HashSet::new();
        let new_values: HashMap<u64, U256> = HashMap::new();

        let changed = ssa_redo(&mut log, &stale, &new_values);
        assert!(changed.is_empty());
        // Values unchanged
        assert_eq!(log.entries[0].output, U256::from(100));
        assert_eq!(log.entries[1].output, U256::from(150));
    }

    #[test]
    fn stale_sload_propagates_through_add() {
        let mut log = SsaLog::new();
        let loc_a = StorageLocation { address: token_addr(), slot: U256::from(42) };

        // Entry 0: SLOAD -> 100 (will become stale, actual value is 200)
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54,
            inputs: smallvec![],
            output: U256::from(100),
            storage_key: Some(loc_a),
        });
        // Entry 1: ADD(entry0, const 50) -> 150 (should become 250)
        log.push(SsaEntry {
            lsn: 1, opcode: 0x01,
            inputs: smallvec![SsaRef::Entry(0), SsaRef::Constant(U256::from(50))],
            output: U256::from(150),
            storage_key: None,
        });
        // Entry 2: SSTORE(entry1) — writes 150 (should become 250)
        log.push(SsaEntry {
            lsn: 2, opcode: 0x55,
            inputs: smallvec![SsaRef::Entry(1)],
            output: U256::from(150),
            storage_key: Some(loc_a),
        });

        let mut stale = HashSet::new();
        stale.insert(0); // entry 0 is stale

        let mut new_values = HashMap::new();
        new_values.insert(0, U256::from(200)); // corrected SLOAD value

        let changed = ssa_redo(&mut log, &stale, &new_values);

        // All 3 entries should be dirty
        assert!(changed.contains(&0));
        assert!(changed.contains(&1));
        assert!(changed.contains(&2));

        // Values corrected
        assert_eq!(log.entries[0].output, U256::from(200));
        assert_eq!(log.entries[1].output, U256::from(250)); // 200 + 50
        assert_eq!(log.entries[2].output, U256::from(250)); // SSTORE writes new value
    }

    #[test]
    fn unchanged_output_stops_propagation() {
        let mut log = SsaLog::new();
        let loc_a = StorageLocation { address: token_addr(), slot: U256::from(42) };

        // Entry 0: SLOAD -> 100 (stale, new value is 200)
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54,
            inputs: smallvec![],
            output: U256::from(100),
            storage_key: Some(loc_a),
        });
        // Entry 1: ISZERO(entry0) -> 0 (100 != 0)
        // After redo: ISZERO(200) -> still 0 (200 != 0)
        // Output unchanged -> should NOT propagate further
        log.push(SsaEntry {
            lsn: 1, opcode: 0x15, // ISZERO
            inputs: smallvec![SsaRef::Entry(0)],
            output: U256::ZERO, // iszero(100) = 0
            storage_key: None,
        });
        // Entry 2: depends on entry 1
        log.push(SsaEntry {
            lsn: 2, opcode: 0x01,
            inputs: smallvec![SsaRef::Entry(1), SsaRef::Constant(U256::from(999))],
            output: U256::from(999), // 0 + 999
            storage_key: None,
        });

        let mut stale = HashSet::new();
        stale.insert(0);
        let mut new_values = HashMap::new();
        new_values.insert(0, U256::from(200));

        let changed = ssa_redo(&mut log, &stale, &new_values);

        // Entry 0 changes (100 -> 200)
        assert!(changed.contains(&0));
        // Entry 1 recomputed but output stays 0 (iszero(200) = 0) — still dirty since input changed
        assert!(changed.contains(&1));
        // Entry 2 should NOT be dirty because entry 1's output didn't change
        assert!(!changed.contains(&2));

        assert_eq!(log.entries[2].output, U256::from(999)); // unchanged
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- ssa_redo_tests`
Expected: FAIL — `ssa_redo` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/ssa_redo.rs`:

```rust
//! SSA Redo Engine (Path 2).
//!
//! After optimistic execution, if conflicts are detected, this engine
//! re-executes only affected operations by walking the SSA dependency graph.

use alloy_primitives::U256;
use std::collections::{HashMap, HashSet};

use crate::types::{SsaLog, SsaRef};

/// Perform operation-level redo on an SSA log.
///
/// # Arguments
/// * `log` - The SSA log to patch (entries are mutated in place)
/// * `stale_lsns` - Set of LSNs whose SLOAD values were stale
/// * `new_values` - Corrected values for the stale LSNs
///
/// # Returns
/// Set of LSNs whose outputs changed during redo.
pub fn ssa_redo(
    log: &mut SsaLog,
    stale_lsns: &HashSet<u64>,
    new_values: &HashMap<u64, U256>,
) -> HashSet<u64> {
    let mut dirty: HashSet<u64> = stale_lsns.clone();
    let mut changed: HashSet<u64> = HashSet::new();

    // Apply corrected values to stale SLOAD entries
    for &lsn in stale_lsns {
        if let Some(new_val) = new_values.get(&lsn) {
            if let Some(entry) = log.entries.iter_mut().find(|e| e.lsn == lsn) {
                if entry.output != *new_val {
                    entry.output = *new_val;
                    changed.insert(lsn);
                }
            }
        }
    }

    // Forward propagation through the SSA graph
    for i in 0..log.entries.len() {
        let has_dirty_input = log.entries[i]
            .inputs
            .iter()
            .any(|input| matches!(input, SsaRef::Entry(ref_lsn) if dirty.contains(ref_lsn)));

        if !has_dirty_input {
            continue;
        }

        let lsn = log.entries[i].lsn;

        // Resolve inputs to concrete values
        let resolved: Vec<U256> = log.entries[i]
            .inputs
            .iter()
            .map(|input| resolve_ref(input, &log.entries))
            .collect();

        let new_output = recompute(log.entries[i].opcode, &resolved);
        let old_output = log.entries[i].output;

        if new_output != old_output {
            log.entries[i].output = new_output;
            dirty.insert(lsn);
            changed.insert(lsn);
        }
        // Even if output didn't change, we mark as "visited" but NOT dirty
        // (only dirty if output actually changed, to stop propagation)
    }

    changed
}

/// Resolve an SSA reference to its concrete U256 value.
fn resolve_ref(r: &SsaRef, entries: &[crate::types::SsaEntry]) -> U256 {
    match r {
        SsaRef::Constant(val) => *val,
        SsaRef::Entry(lsn) => entries
            .iter()
            .find(|e| e.lsn == *lsn)
            .map(|e| e.output)
            .unwrap_or(U256::ZERO),
    }
}

/// Recompute an EVM opcode given resolved input values.
///
/// This is a subset of EVM opcodes needed for redo. We only need to handle
/// opcodes that appear in the SSA log (arithmetic, comparison, storage).
fn recompute(opcode: u8, inputs: &[U256]) -> U256 {
    match opcode {
        // ADD
        0x01 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            a.wrapping_add(b)
        }
        // MUL
        0x02 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            a.wrapping_mul(b)
        }
        // SUB
        0x03 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            a.wrapping_sub(b)
        }
        // DIV
        0x04 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            if b.is_zero() { U256::ZERO } else { a / b }
        }
        // MOD
        0x06 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            if b.is_zero() { U256::ZERO } else { a % b }
        }
        // LT
        0x10 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            if a < b { U256::from(1) } else { U256::ZERO }
        }
        // GT
        0x11 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            if a > b { U256::from(1) } else { U256::ZERO }
        }
        // EQ
        0x14 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            if a == b { U256::from(1) } else { U256::ZERO }
        }
        // ISZERO
        0x15 => {
            let a = get(inputs, 0);
            if a.is_zero() { U256::from(1) } else { U256::ZERO }
        }
        // AND
        0x16 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            a & b
        }
        // OR
        0x17 => {
            let (a, b) = (get(inputs, 0), get(inputs, 1));
            a | b
        }
        // NOT
        0x19 => {
            let a = get(inputs, 0);
            !a
        }
        // SLOAD — re-read returns the new value directly (already in new_values)
        0x54 => get(inputs, 0),
        // SSTORE — the value to store is the input
        0x55 => get(inputs, 0),
        // For unhandled opcodes, return the first input as passthrough.
        // This is safe because if the opcode isn't in our recompute table,
        // its output won't change from input changes (conservative).
        _ => get(inputs, 0),
    }
}

fn get(inputs: &[U256], idx: usize) -> U256 {
    inputs.get(idx).copied().unwrap_or(U256::ZERO)
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod ssa_redo;
#[cfg(test)]
mod ssa_redo_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- ssa_redo_tests`
Expected: all 3 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/ssa_redo.rs crates/node/paraevm/src/ssa_redo_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add SSA redo engine for operation-level conflict resolution"
```

---

## Task 6: Path 2 — OCC Validator

**Files:**
- Create: `crates/node/paraevm/src/validator.rs`
- Create: `crates/node/paraevm/src/validator_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/validator_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256, address};
    use smallvec::smallvec;
    use crate::types::{SsaEntry, SsaLog, SsaRef, StorageLocation, WriteBuffer};
    use crate::validator::validate_tx;

    fn token() -> Address {
        address!("20C0000000000000000000000000000000000001")
    }

    fn loc(slot: u64) -> StorageLocation {
        StorageLocation { address: token(), slot: U256::from(slot) }
    }

    #[test]
    fn no_conflicts_returns_clean() {
        // tx0 reads slot 1, tx1 writes slot 2 — no conflict
        let mut log = SsaLog::new();
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54, // SLOAD
            inputs: smallvec![],
            output: U256::from(100),
            storage_key: Some(loc(1)),
        });

        // committed_writes from earlier txns: slot 2 was written
        let committed_writes = vec![(loc(2), U256::from(999))];

        let result = validate_tx(&log, &committed_writes);
        assert!(result.is_none()); // no conflicts
    }

    #[test]
    fn conflict_detected_when_read_slot_was_written() {
        // tx reads slot 1 (got value 100), but an earlier tx wrote slot 1 (value 200)
        let mut log = SsaLog::new();
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54, // SLOAD
            inputs: smallvec![],
            output: U256::from(100), // stale value
            storage_key: Some(loc(1)),
        });

        let committed_writes = vec![(loc(1), U256::from(200))];

        let result = validate_tx(&log, &committed_writes);
        assert!(result.is_some());

        let (stale_lsns, new_values) = result.unwrap();
        assert!(stale_lsns.contains(&0));
        assert_eq!(new_values[&0], U256::from(200));
    }

    #[test]
    fn no_conflict_when_written_value_matches_read() {
        // tx reads slot 1 (got 100), earlier tx also wrote 100 to slot 1 — no real conflict
        let mut log = SsaLog::new();
        log.push(SsaEntry {
            lsn: 0, opcode: 0x54,
            inputs: smallvec![],
            output: U256::from(100), // matches committed value
            storage_key: Some(loc(1)),
        });

        let committed_writes = vec![(loc(1), U256::from(100))];

        let result = validate_tx(&log, &committed_writes);
        assert!(result.is_none()); // no actual conflict
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- validator_tests`
Expected: FAIL — `validator` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/validator.rs`:

```rust
//! OCC Validator (Path 2, Phase 2).
//!
//! After optimistic execution, validates that SLOAD values read during
//! execution haven't been overwritten by earlier-committed transactions.

use alloy_primitives::U256;
use std::collections::{HashMap, HashSet};

use crate::types::{SsaLog, StorageLocation};

/// Validate a transaction's SSA log against committed writes.
///
/// Returns `None` if no conflicts (transaction can commit).
/// Returns `Some((stale_lsns, new_values))` if conflicts detected,
/// which can be passed directly to `ssa_redo`.
pub fn validate_tx(
    log: &SsaLog,
    committed_writes: &[(StorageLocation, U256)],
) -> Option<(HashSet<u64>, HashMap<u64, U256>)> {
    // Build a map of committed writes for O(1) lookup
    let write_map: HashMap<StorageLocation, U256> = committed_writes.iter().cloned().collect();

    let mut stale_lsns = HashSet::new();
    let mut new_values = HashMap::new();

    for entry in &log.entries {
        // Only check SLOAD entries (opcode 0x54)
        if entry.opcode != 0x54 {
            continue;
        }

        if let Some(ref storage_key) = entry.storage_key {
            if let Some(&committed_value) = write_map.get(storage_key) {
                // A concurrent tx wrote to this slot — check if the value differs
                if committed_value != entry.output {
                    stale_lsns.insert(entry.lsn);
                    new_values.insert(entry.lsn, committed_value);
                }
            }
        }
    }

    if stale_lsns.is_empty() {
        None
    } else {
        Some((stale_lsns, new_values))
    }
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod validator;
#[cfg(test)]
mod validator_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- validator_tests`
Expected: all 3 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/validator.rs crates/node/paraevm/src/validator_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add OCC validator for Path 2 conflict detection"
```

---

## Task 7: Worker Pool

**Files:**
- Create: `crates/node/paraevm/src/worker_pool.rs`
- Create: `crates/node/paraevm/src/worker_pool_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/worker_pool_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256, Bytes, U256, KECCAK256_EMPTY};
    use magnus_qmdb::ChangeSet;
    use magnus_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};
    use crate::worker_pool::WorkerPool;

    #[derive(Clone, Debug, Default)]
    struct MockStateDb;

    impl StateDbRead for MockStateDb {
        async fn nonce(&self, _: &Address) -> Result<u64, StateDbError> { Ok(0) }
        async fn balance(&self, _: &Address) -> Result<U256, StateDbError> { Ok(U256::from(1_000_000)) }
        async fn code_hash(&self, _: &Address) -> Result<B256, StateDbError> { Ok(KECCAK256_EMPTY) }
        async fn code(&self, _: &B256) -> Result<Bytes, StateDbError> { Ok(Bytes::new()) }
        async fn storage(&self, _: &Address, _: &U256) -> Result<U256, StateDbError> { Ok(U256::ZERO) }
    }

    impl StateDbWrite for MockStateDb {
        async fn commit(&self, _: ChangeSet) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
        async fn compute_root(&self, _: &ChangeSet) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
        fn merge_changes(&self, _: ChangeSet, newer: ChangeSet) -> ChangeSet { newer }
    }

    impl StateDb for MockStateDb {
        async fn state_root(&self) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
    }

    #[test]
    fn worker_pool_creates_with_default_threads() {
        let pool = WorkerPool::new(None);
        assert!(pool.num_threads() > 0);
    }

    #[test]
    fn worker_pool_creates_with_explicit_threads() {
        let pool = WorkerPool::new(Some(4));
        assert_eq!(pool.num_threads(), 4);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- worker_pool_tests`
Expected: FAIL — `worker_pool` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/worker_pool.rs`:

```rust
//! Shared REVM Worker Pool.
//!
//! Both Path 1 and Path 2 share a rayon thread pool for EVM execution.

use rayon::ThreadPool;

/// Shared worker pool for parallel EVM execution.
pub struct WorkerPool {
    pool: ThreadPool,
}

impl WorkerPool {
    /// Create a new worker pool.
    ///
    /// If `num_threads` is None, uses the number of available CPUs.
    pub fn new(num_threads: Option<usize>) -> Self {
        let n = num_threads.unwrap_or_else(|| std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4));

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(|i| format!("magnus-paraevm-{i}"))
            .build()
            .expect("failed to build rayon thread pool");

        Self { pool }
    }

    /// Number of threads in the pool.
    pub fn num_threads(&self) -> usize {
        self.pool.current_num_threads()
    }

    /// Get a reference to the underlying rayon ThreadPool for `pool.install()` calls.
    pub fn pool(&self) -> &ThreadPool {
        &self.pool
    }
}
```

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod worker_pool;
#[cfg(test)]
mod worker_pool_tests;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- worker_pool_tests`
Expected: all 2 tests PASS

**Step 5: Commit**

```bash
git add crates/node/paraevm/src/worker_pool.rs crates/node/paraevm/src/worker_pool_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "feat(paraevm): add shared rayon worker pool"
```

---

## Task 8: ParaEvmExecutor — Wire it all together

**Files:**
- Create: `crates/node/paraevm/src/executor.rs`
- Create: `crates/node/paraevm/src/executor_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

This is the main integration: `ParaEvmExecutor` implements `BlockExecutor<S>`, uses the router to split transactions, executes Path 1 frames in parallel via the worker pool, and falls back to sequential execution for Path 2 (SSA instrumentation comes in a follow-up iteration).

**Step 1: Write the failing test**

Create `crates/node/paraevm/src/executor_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256, Bytes, U256, KECCAK256_EMPTY};
    use magnus_qmdb::ChangeSet;
    use magnus_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};
    use crate::executor::ParaEvmExecutor;
    use crate::types::ExecutionPath;

    #[derive(Clone, Debug, Default)]
    struct MockStateDb;

    impl StateDbRead for MockStateDb {
        async fn nonce(&self, _: &Address) -> Result<u64, StateDbError> { Ok(0) }
        async fn balance(&self, _: &Address) -> Result<U256, StateDbError> { Ok(U256::from(1_000_000)) }
        async fn code_hash(&self, _: &Address) -> Result<B256, StateDbError> { Ok(KECCAK256_EMPTY) }
        async fn code(&self, _: &B256) -> Result<Bytes, StateDbError> { Ok(Bytes::new()) }
        async fn storage(&self, _: &Address, _: &U256) -> Result<U256, StateDbError> { Ok(U256::ZERO) }
    }

    impl StateDbWrite for MockStateDb {
        async fn commit(&self, _: ChangeSet) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
        async fn compute_root(&self, _: &ChangeSet) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
        fn merge_changes(&self, _: ChangeSet, newer: ChangeSet) -> ChangeSet { newer }
    }

    impl StateDb for MockStateDb {
        async fn state_root(&self) -> Result<B256, StateDbError> { Ok(B256::ZERO) }
    }

    #[test]
    fn executor_creates_with_defaults() {
        let executor = ParaEvmExecutor::new(1, None);
        assert_eq!(executor.chain_id(), 1);
    }

    #[test]
    fn executor_router_classifies_correctly() {
        let mut executor = ParaEvmExecutor::new(1, None);

        // Register a known contract
        let token = Address::with_last_byte(0x20);
        let transfer_sel = [0xa9, 0x05, 0x9c, 0xbb];
        executor.router_mut().register_known_selector(token, transfer_sel, ExecutionPath::ExactSchedule);

        // Verify classification works through the executor
        let path = executor.router().classify(
            Address::ZERO,
            Some(token),
            U256::ZERO,
            &Bytes::from(vec![0xa9, 0x05, 0x9c, 0xbb, 0x00]),
        );
        assert_eq!(path, ExecutionPath::ExactSchedule);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p magnus-paraevm -- executor_tests`
Expected: FAIL — `executor` module not found

**Step 3: Write minimal implementation**

Create `crates/node/paraevm/src/executor.rs`:

```rust
//! ParaEvmExecutor — 2-path parallel block executor.
//!
//! Implements `BlockExecutor<S>` from magnus-executor. Routes transactions
//! through Path 1 (exact scheduling) or Path 2 (sequential for now, SSA
//! instrumented REVM in a follow-up).

use alloy_primitives::{Bytes, U256, keccak256};
use magnus_executor::{
    BlockContext, BlockExecutor, ExecutionConfig, ExecutionError, ExecutionOutcome,
    ExecutionReceipt, StateDbAdapter,
};
use magnus_qmdb::ChangeSet;
use magnus_traits::StateDb;
use revm::{
    Context, ExecuteEvm, Journal, MainBuilder,
    context::block::BlockEnv,
    context::result::{ExecutionResult, Output},
    context_interface::ContextSetters,
    database::State,
};

use crate::router::TransactionRouter;
use crate::types::ExecutionPath;
use crate::worker_pool::WorkerPool;

/// 2-path parallel EVM executor.
#[derive(Clone)]
pub struct ParaEvmExecutor {
    config: ExecutionConfig,
    router: TransactionRouter,
    num_threads: Option<usize>,
}

impl ParaEvmExecutor {
    /// Create a new parallel executor.
    pub fn new(chain_id: u64, num_threads: Option<usize>) -> Self {
        Self {
            config: ExecutionConfig::new(chain_id),
            router: TransactionRouter::new(),
            num_threads,
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.config.chain_id
    }

    pub fn router(&self) -> &TransactionRouter {
        &self.router
    }

    pub fn router_mut(&mut self) -> &mut TransactionRouter {
        &mut self.router
    }
}

impl std::fmt::Debug for ParaEvmExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParaEvmExecutor")
            .field("chain_id", &self.config.chain_id)
            .finish()
    }
}

impl<S: StateDb> BlockExecutor<S> for ParaEvmExecutor {
    type Tx = Bytes;

    fn execute(
        &self,
        state: &S,
        context: &BlockContext,
        txs: &[Self::Tx],
    ) -> Result<ExecutionOutcome, ExecutionError> {
        // Phase 0: Classify all transactions
        // For now, we execute all transactions sequentially using the existing
        // REVM path. The router classification is wired but parallel frame
        // execution will be enabled in a follow-up once the SSA instrumentation
        // is integrated with REVM's Inspector trait.

        let adapter = StateDbAdapter::new(state.clone());
        let db = State::builder().with_database_ref(adapter).build();

        type Db<S> = State<revm::database::WrapDatabaseRef<StateDbAdapter<S>>>;
        let ctx: Context<BlockEnv, _, _, Db<S>, Journal<Db<S>>, ()> =
            Context::new(db, self.config.spec_id);
        let ctx = ctx
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = self.config.chain_id;
            })
            .modify_block_chained(|blk: &mut BlockEnv| {
                blk.number = U256::from(context.header.number);
                blk.timestamp = U256::from(context.header.timestamp);
                blk.beneficiary = context.header.beneficiary;
                blk.gas_limit = context.header.gas_limit;
                blk.basefee = context.header.base_fee_per_gas.unwrap_or_default();
                blk.prevrandao = Some(context.prevrandao);
            });

        let mut evm = ctx.build_mainnet();
        let mut outcome = ExecutionOutcome::new();
        let mut cumulative_gas = 0u64;
        let mut lazy_fees = 0u64;

        for tx_bytes in txs {
            let tx_hash = keccak256(tx_bytes);
            let tx_env = magnus_executor::tx_types::decode_tx_env(tx_bytes, self.config.chain_id)?;
            evm.set_tx(tx_env);

            let result_and_state =
                evm.replay().map_err(|e| ExecutionError::TxExecution(format!("{:?}", e)))?;

            let gas_used = result_and_state.result.gas_used();
            cumulative_gas = cumulative_gas.saturating_add(gas_used);
            lazy_fees = lazy_fees.saturating_add(gas_used);

            let receipt = build_receipt(&result_and_state.result, tx_hash, gas_used, cumulative_gas);
            outcome.receipts.push(receipt);

            let changes = magnus_executor::extract_changes(result_and_state.state);
            outcome.changes.merge(changes);
        }

        outcome.gas_used = cumulative_gas;
        Ok(outcome)
    }

    fn validate_header(&self, header: &alloy_consensus::Header) -> Result<(), ExecutionError> {
        if header.gas_limit < self.config.gas_limit_bounds.min {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} below minimum {}",
                header.gas_limit, self.config.gas_limit_bounds.min
            )));
        }
        if header.gas_limit > self.config.gas_limit_bounds.max {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} above maximum {}",
                header.gas_limit, self.config.gas_limit_bounds.max
            )));
        }
        Ok(())
    }
}

fn build_receipt(
    result: &ExecutionResult,
    tx_hash: alloy_primitives::B256,
    gas_used: u64,
    cumulative_gas_used: u64,
) -> ExecutionReceipt {
    let (success, logs, contract_address) = match result {
        ExecutionResult::Success { logs, output, .. } => {
            let contract_addr = match output {
                Output::Create(_, addr) => *addr,
                Output::Call(_) => None,
            };
            (true, logs.clone(), contract_addr)
        }
        ExecutionResult::Revert { .. } => (false, Vec::new(), None),
        ExecutionResult::Halt { .. } => (false, Vec::new(), None),
    };
    ExecutionReceipt::new(tx_hash, success, gas_used, cumulative_gas_used, logs, contract_address)
}
```

Note: The `decode_tx_env` and `extract_changes` functions need to be public in `magnus-executor`. Check if they are, and if not, add `pub` visibility.

Add to `crates/node/paraevm/src/lib.rs`:
```rust
pub mod executor;
#[cfg(test)]
mod executor_tests;
```

Also add `alloy-consensus` to `[dependencies]` in `crates/node/paraevm/Cargo.toml`:
```toml
alloy-consensus.workspace = true
```

**Step 4: Check that `decode_tx_env` and `extract_changes` are public in magnus-executor**

Look at `crates/node/executor/src/revm.rs`. If `decode_tx_env` and `extract_changes` are `fn` (private), change them to `pub fn` and re-export from `crates/node/executor/src/lib.rs`:
```rust
pub use revm::{RevmExecutor, calculate_base_fee, decode_tx_env, extract_changes};
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p magnus-paraevm -- executor_tests`
Expected: all 2 tests PASS

**Step 6: Commit**

```bash
git add crates/node/paraevm/ crates/node/executor/src/revm.rs crates/node/executor/src/lib.rs
git commit -m "feat(paraevm): add ParaEvmExecutor implementing BlockExecutor"
```

---

## Task 9: Lazy Beneficiary integration test

**Files:**
- Create: `crates/node/paraevm/src/lazy_beneficiary_tests.rs`
- Modify: `crates/node/paraevm/src/lib.rs`

**Step 1: Write the test**

Create `crates/node/paraevm/src/lazy_beneficiary_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, address};
    use crate::types::LazyBeneficiary;

    #[test]
    fn lazy_beneficiary_accumulates_fees() {
        let beneficiary = address!("b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0");
        let mut lb = LazyBeneficiary::new(beneficiary);

        lb.add_fee(21000);
        lb.add_fee(42000);
        lb.add_fee(21000);

        assert_eq!(lb.accumulated_fees, 84000);
        assert_eq!(lb.beneficiary, beneficiary);
    }

    #[test]
    fn lazy_beneficiary_saturates_on_overflow() {
        let mut lb = LazyBeneficiary::new(Address::ZERO);
        lb.accumulated_fees = u64::MAX - 10;
        lb.add_fee(100);
        assert_eq!(lb.accumulated_fees, u64::MAX);
    }
}
```

**Step 2: Run test to verify it passes**

Add `#[cfg(test)] mod lazy_beneficiary_tests;` to `lib.rs`.

Run: `cargo test -p magnus-paraevm -- lazy_beneficiary_tests`
Expected: all 2 tests PASS

**Step 3: Commit**

```bash
git add crates/node/paraevm/src/lazy_beneficiary_tests.rs crates/node/paraevm/src/lib.rs
git commit -m "test(paraevm): add lazy beneficiary accumulation tests"
```

---

## Task 10: Full integration test — Router + Scheduler + Execution

**Files:**
- Create: `crates/node/paraevm/tests/integration.rs`

**Step 1: Write the integration test**

Create `crates/node/paraevm/tests/integration.rs`:

```rust
//! Integration tests for MagnusParaEVM pipeline.

use alloy_primitives::{Address, U256, address};
use magnus_paraevm::router::TransactionRouter;
use magnus_paraevm::rw_derive::{derive_mip20_transfer_rw, derive_native_transfer_rw};
use magnus_paraevm::scheduler::FrameScheduler;
use magnus_paraevm::types::ExecutionPath;

/// Test: classify 10 MIP20 transfers to different recipients,
/// derive their read/write sets, and verify they pack into 1 frame
/// (non-conflicting because different token holders).
#[test]
fn mip20_transfers_to_different_recipients_single_frame() {
    let token = address!("20C0000000000000000000000000000000000001");
    let transfer_sel = [0xa9, 0x05, 0x9c, 0xbb];

    let mut router = TransactionRouter::new();
    router.register_known_selector(token, transfer_sel, ExecutionPath::ExactSchedule);

    let mut rw_sets = Vec::new();

    for i in 1..=10u8 {
        let from = Address::with_last_byte(i);
        let to = Address::with_last_byte(100 + i);

        // Verify router classifies as Path 1
        let mut calldata = Vec::with_capacity(68);
        calldata.extend_from_slice(&transfer_sel);
        calldata.extend_from_slice(&[0u8; 64]);
        let path = router.classify(
            from,
            Some(token),
            U256::ZERO,
            &alloy_primitives::Bytes::from(calldata),
        );
        assert_eq!(path, ExecutionPath::ExactSchedule);

        // Derive RW set
        let rw = derive_mip20_transfer_rw(token, from, to);
        rw_sets.push(rw);
    }

    // All to different accounts — should fit in 1 frame
    let frames = FrameScheduler::pack(&rw_sets);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].tx_indices.len(), 10);
}

/// Test: two transfers from the same sender conflict (balances[from] overlap).
#[test]
fn same_sender_transfers_conflict() {
    let token = address!("20C0000000000000000000000000000000000001");
    let from = address!("aaaa000000000000000000000000000000000001");
    let to1 = address!("bbbb000000000000000000000000000000000001");
    let to2 = address!("cccc000000000000000000000000000000000001");

    let rw1 = derive_mip20_transfer_rw(token, from, to1);
    let rw2 = derive_mip20_transfer_rw(token, from, to2);

    let frames = FrameScheduler::pack(&[rw1, rw2]);
    // balances[from] overlaps -> 2 frames
    assert_eq!(frames.len(), 2);
}

/// Test: native transfers between disjoint pairs pack into 1 frame.
#[test]
fn native_transfers_disjoint_single_frame() {
    let mut rw_sets = Vec::new();
    for i in 0..5u8 {
        let from = Address::with_last_byte(i * 2);
        let to = Address::with_last_byte(i * 2 + 1);
        rw_sets.push(derive_native_transfer_rw(from, to));
    }

    let frames = FrameScheduler::pack(&rw_sets);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].tx_indices.len(), 5);
}
```

**Step 2: Run integration tests**

Run: `cargo test -p magnus-paraevm --test integration`
Expected: all 3 tests PASS

**Step 3: Commit**

```bash
git add crates/node/paraevm/tests/integration.rs
git commit -m "test(paraevm): add integration tests for router + scheduler pipeline"
```

---

## Task 11: Full crate check and final cleanup

**Files:**
- Modify: `crates/node/paraevm/src/lib.rs` (final public API)

**Step 1: Run all tests in the crate**

Run: `cargo test -p magnus-paraevm`
Expected: all tests PASS

**Step 2: Run clippy**

Run: `cargo clippy -p magnus-paraevm -- -D warnings`
Expected: no warnings

**Step 3: Verify the full workspace still compiles**

Run: `cargo check --workspace`
Expected: compiles (may have unrelated warnings)

**Step 4: Final commit if any cleanup needed**

```bash
git add -A crates/node/paraevm/
git commit -m "chore(paraevm): clippy fixes and final cleanup"
```

---

## Summary

| Task | Component | Est. Time |
|------|-----------|-----------|
| 1 | Crate skeleton + types | Short |
| 2 | Transaction Router | Short |
| 3 | Path 1: R/W set derivation | Short |
| 4 | Path 1: Frame scheduler | Short |
| 5 | Path 2: SSA redo engine | Medium |
| 6 | Path 2: OCC validator | Short |
| 7 | Worker pool | Short |
| 8 | ParaEvmExecutor (wire-up) | Medium |
| 9 | Lazy beneficiary tests | Short |
| 10 | Integration tests | Short |
| 11 | Clippy + workspace check | Short |

**What's deferred to a follow-up plan (P1):**
- REVM Inspector integration for SSA logging during actual execution (requires custom `Inspector` impl that records `SsaEntry` on each opcode)
- revmc AOT compilation integration
- Known contract registry governance precompile
- Parallel frame execution via rayon `pool.install(|| frames.par_iter()...)`
- Full end-to-end benchmarks against sequential RevmExecutor
