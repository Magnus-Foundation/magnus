# MagnusParaEVM: 2-Path Parallel Execution Engine

**Date:** 2026-02-18
**Status:** Approved
**Replaces:** FAFO (Bloom filter pre-scheduling) from Magnus Hybrid Execution Strategy

## Problem Statement

The original Magnus Hybrid Execution Strategy proposed FAFO (Fast Access Filter Oracle) using Bloom filters to pre-schedule transactions for parallel execution. FAFO has fundamental limitations:

- **8% false positive rate** on Bloom filter conflict detection causes unnecessary serialization
- **Cannot handle complex contracts** with dynamic storage access patterns
- **Static analysis only** - misses runtime-dependent state access (e.g., mapping lookups dependent on calldata)

We need a parallel execution engine that:
1. Achieves zero false positives for known payment transactions (70% of Magnus workload)
2. Handles arbitrary unknown contracts without pre-analysis
3. Minimizes re-execution overhead on conflicts

## Architecture Overview

MagnusParaEVM uses a **2-path architecture** with a Transaction Router that classifies transactions at block construction time:

```
                    Pending Transactions
                           |
                   [Transaction Router]
                    /              \
              Known Txns        Unknown Txns
                  |                  |
         [Path 1: Exact         [Path 2: OpLevel
          Scheduler]             OCC + SSA Redo]
                  \                /
               [Shared REVM Worker Pool]
                        |
                   [QMDB State]
```

**Path 1 (Exact Scheduler):** For known contracts (MIP-20 tokens, native transfers, DEX swaps). Uses HashSet-based read/write set intersection for conflict detection. Zero false positives, zero SSA overhead.

**Path 2 (Operation-Level OCC with SSA Redo):** For unknown/complex contracts. Inspired by ParallelEVM (EuroSys 2025, arXiv:2211.07911). Logs EVM operations at opcode granularity using SSA (Static Single Assignment). On conflict, re-executes only affected operations instead of entire transactions.

### Why 2 Paths Instead of 1

| Concern | Single Path (OpLevel OCC only) | 2-Path (Recommended) |
|---------|-------------------------------|---------------------|
| SSA overhead on simple txns | 15-20% overhead on every transfer | Zero overhead for 70% of txns |
| Payment TPS | ~1.8M | ~2.1-2.5M |
| Complexity | Lower | Moderate (router + 2 schedulers) |
| Unknown contract handling | Excellent | Excellent (same Path 2) |

The 70/30 payment-heavy workload on Magnus makes the 2-path design clearly superior for the target use case.

## Component Design

### 1. Transaction Router

Classifies transactions in O(1) per transaction using a known contract registry.

```rust
enum ExecutionPath {
    ExactSchedule,   // Path 1: known txns, pre-computed read/write sets
    OpLevelOCC,      // Path 2: unknown txns, SSA logging + redo
}

struct TransactionRouter {
    /// Contracts with known storage layouts (MIP-20, DEX, etc.)
    known_contracts: HashSet<Address>,
    /// Per-selector path overrides (some selectors on known contracts
    /// may still need Path 2 if they have dynamic behavior)
    known_selectors: HashMap<(Address, [u8; 4]), ExecutionPath>,
    /// LRU cache for EOA-to-EOA native transfers (always Path 1)
    eoa_cache: LruCache<Address, bool>,
}
```

**Classification rules:**
1. Native transfer (no calldata, value > 0) -> Path 1
2. Known contract + known selector -> Path 1
3. Known contract + unknown selector -> Path 2
4. Unknown contract -> Path 2

The known contract registry is populated at genesis for system precompiles (MIP-20, MIP-403, Fee Manager) and extended via governance for verified contracts (e.g., Uniswap V3 deployments).

### 2. Path 1: Exact Scheduler

For transactions routed to Path 1, the scheduler pre-computes exact read/write sets from the transaction's target contract and calldata, then packs non-conflicting transactions into parallel frames.

**Read/Write Set Derivation:**

For known contracts, storage slot formulas are deterministic:
- MIP-20 `transfer(to, amount)`: reads/writes `balances[from]`, `balances[to]`
- Native transfer: reads/writes `balance[from]`, `balance[to]`
- MIP-20 `approve(spender, amount)`: writes `allowances[from][spender]`

Storage slots are computed using the same keccak256 layout as the precompile storage system.

**Frame-Based Greedy Packing (O(n)):**

```
Algorithm: GreedyFramePack
Input: transactions T[], read/write sets RW[]
Output: frames F[] (each frame executes in parallel)

occupied = HashSet::new()   // slots claimed in current frame
current_frame = Frame::new()

for tx in T:
    if occupied.intersects(RW[tx]):
        F.push(current_frame)
        occupied.clear()
        current_frame = Frame::new()
    current_frame.add(tx)
    occupied.extend(RW[tx])

F.push(current_frame)
```

This is a single-pass O(n) algorithm. Transactions within a frame execute in parallel across the worker pool. Frames execute sequentially. For payment-heavy workloads, frame density is high (most transfers touch different accounts).

### 3. Path 2: Operation-Level OCC with SSA Redo

For transactions routed to Path 2, execution uses optimistic concurrency control at the EVM opcode level, inspired by the ParallelEVM paper.

#### 3.1 SSA Operation Log

Every EVM opcode execution is logged as an SSA entry:

```rust
struct SsaEntry {
    /// Log sequence number (monotonically increasing per transaction)
    lsn: u64,
    /// The EVM opcode
    opcode: Opcode,
    /// Input references: either prior log entries or constants
    inputs: SmallVec<[SsaRef; 4]>,
    /// Output value produced by this operation
    output: U256,
    /// For SLOAD/SSTORE: the storage key accessed
    storage_key: Option<StorageKey>,
}

enum SsaRef {
    /// Reference to a prior SSA entry's output
    Entry(u64),
    /// A constant value (from PUSH, calldata, etc.)
    Constant(U256),
}

struct StorageKey {
    address: Address,
    slot: U256,
}
```

**Shadow Stack:** A parallel stack that mirrors the EVM operand stack, mapping each stack position to the LSN of the SSA entry that produced the value (or `None` for constants pushed directly).

```rust
struct SsaLog {
    entries: Vec<SsaEntry>,
    shadow_stack: Vec<Option<u64>>,  // maps stack positions -> LSN
}
```

#### 3.2 Three-Phase Execution

**Phase 1: Optimistic Execution**
- All Path 2 transactions execute concurrently against a snapshot of state
- Each transaction's REVM instance is instrumented to record SSA entries
- SLOAD reads from snapshot (may read stale data)
- SSTORE writes to a thread-local write buffer

**Phase 2: Validation**
- After all transactions complete, validate read sets
- For each SLOAD entry in the SSA log, check if the storage slot was written by a concurrent transaction that committed earlier in block order
- If no conflicts: commit write buffer to state
- If conflict detected: mark transaction for redo

**Phase 3: Operation-Level Redo**
- For conflicted transactions, re-read only the stale SLOAD values
- Walk the SSA dependency graph forward from the changed SLOAD entries
- Re-execute only operations whose inputs changed (transitively)
- This is the key advantage over transaction-level OCC: instead of re-executing the entire transaction, only ~5-15% of operations need re-execution on typical conflicts

**Redo Algorithm:**
```
Algorithm: SSA_Redo
Input: ssa_log, stale_sloads (set of LSNs with changed values)

dirty = stale_sloads.clone()
for entry in ssa_log.entries:
    if any(input in dirty for input in entry.inputs):
        new_output = recompute(entry.opcode, resolve_inputs(entry.inputs))
        if new_output != entry.output:
            entry.output = new_output
            dirty.insert(entry.lsn)
```

### 4. Shared REVM Worker Pool

Both paths share a common pool of REVM executor threads:

- **Pool size:** `num_cpus` workers (default), configurable
- **revmc AOT:** Known hot contracts (MIP-20, DEX routers) are compiled to native code via revmc ahead-of-time compilation. Workers use AOT-compiled bytecode when available, falling back to REVM interpretation.
- **State access:** All workers read/write through QMDB's concurrent state interface

### 5. Lazy Beneficiary

Block reward / fee distribution to the proposer is deferred until after all transaction execution completes. This eliminates the proposer's balance as a universal write conflict (every transaction pays fees to the same address).

```rust
struct LazyBeneficiary {
    beneficiary: Address,
    accumulated_fees: AtomicU64,  // summed after all txns
}
```

Applied as a single final state mutation after the parallel execution phase.

### 6. QMDB Integration

State reads and writes go through QMDB (Quantum Merkle Database), which provides:
- **Concurrent read access** via immutable state snapshots
- **Batch write commits** for frame/epoch boundaries
- **O(1) state proof generation** for light clients

## Performance Projections

Based on ParallelEVM benchmarks (4.28x speedup on Ethereum mainnet traces) and Magnus's payment-heavy workload profile:

| Metric | Estimate | Basis |
|--------|----------|-------|
| Path 1 speedup (16 cores) | ~12-14x | High parallelism, minimal conflicts in payment txns |
| Path 2 speedup (16 cores) | ~4-5x | ParallelEVM paper: 4.28x on diverse workloads |
| Blended speedup (70/30 split) | ~9-11x | Weighted average |
| Projected TPS (16 cores) | ~2.1-2.5M | From ~200-250K sequential baseline |
| SSA logging overhead | ~15-20% per txn | Only on Path 2 (30% of transactions) |
| Redo vs full re-execution | ~5-15% ops | ParallelEVM: median 8% of operations re-executed |

## Implementation Scope

Estimated implementation size: ~6,000-8,000 lines of Rust

| Component | Est. Lines | Priority |
|-----------|-----------|----------|
| Transaction Router | ~400 | P0 |
| Path 1: Exact Scheduler | ~800 | P0 |
| Path 2: SSA Log + Shadow Stack | ~1,200 | P0 |
| Path 2: OCC Validator | ~600 | P0 |
| Path 2: SSA Redo Engine | ~800 | P0 |
| Shared Worker Pool | ~500 | P0 |
| Lazy Beneficiary | ~200 | P0 |
| revmc AOT Integration | ~600 | P1 |
| Known Contract Registry (governance) | ~400 | P1 |
| Benchmarks + Tests | ~2,000 | P0 |

## References

- **ParallelEVM:** Xingda Wei et al., "Scaling Blockchain Execution with Operation-Level Concurrency," EuroSys 2025 (arXiv:2211.07911). Key technique: SSA operation log with shadow stack for operation-level conflict detection and redo.
- **Block-STM:** Aptos Labs, multi-version data structure with optimistic scheduling.
- **Sei v2 OCC:** Optimistic concurrency control at transaction level.
- **revmc:** Paradigm's ahead-of-time EVM bytecode compiler to native code.
- **QMDB:** Magnus Chain's Quantum Merkle Database for O(1) state proofs.

## Decision Log

| Decision | Options Considered | Choice | Rationale |
|----------|-------------------|--------|-----------|
| Replace FAFO | Keep FAFO, new approach | New approach | Bloom filter 8% FP rate, can't handle complex contracts |
| Conflict detection | Bloom, Cuckoo, Xor, Binary Fuse, HashSet, Bonsai Acc, Conflict Specs, OpLevel | OpLevel OCC + HashSet | Best of both: exact for known, opcode-level for unknown |
| Architecture | Single path, 2-path | 2-path | 70% payment workload gets zero SSA overhead |
| Path 2 granularity | Transaction-level OCC, Operation-level OCC | Operation-level | 4.28x vs 2.49x speedup (ParallelEVM benchmarks) |
