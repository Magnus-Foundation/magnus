# Magnus V4: Plan 5-A Final Specification

## Executive Summary

Magnus is a Vietnam-focused payment L1 blockchain forked from Tempo. This document specifies the complete architecture restructuring following Plan 5-A (Enterprise + Abstraction) to ensure:

1. **Complete identity separation** from Tempo
2. **Abstraction layers** hiding implementation details (FAFO, Simplex, QMDB)
3. **Enterprise-grade folder structure** for team scaling
4. **Multi-fiat stablecoin support** (VNST, EURC, USDC)

---

## 1. Naming Convention

### 1.1 Symbol Renaming Rules

| Original | Renamed | Notes |
|----------|---------|-------|
| `tempo` | `magnus` | All lowercase |
| `Tempo` | `Magnus` | PascalCase |
| `TEMPO` | `MAGNUS` | SCREAMING_CASE |
| `TIP-20` | `MIP-20` | Magnus Improvement Proposal |
| `TIP-403` | `MIP-403` | Transfer Policy Registry |
| `TIP_FEE_MANAGER` | `MIP_FEE_MANAGER` | Precompile constant |
| `tempoup` | `magnusup` | Installer tool |
| `tempo-node` | `magnus-node` | Binary name |
| `tempo-sidecar` | `magnus-sidecar` | Binary name |
| `tempo-bench` | `magnus-bench` | Binary name |

### 1.2 RPC Namespace Renaming

| Original | Renamed |
|----------|---------|
| `tempo_*` | `magnus_*` |
| `tempo_getBlockByNumber` | `magnus_getBlockByNumber` |
| `tempo_sendTransaction` | `magnus_sendTransaction` |

### 1.3 Files to NOT Rename

- Third-party dependencies (revm, alloy, etc.)
- Standard EVM/Ethereum terminology (EVM, ERC, EIP)
- Generic terms that happen to contain "tempo" in other contexts

---

## 2. Folder Structure (42 Crates)

```
magnus/
├── apps/                           # Application binaries
│   ├── node/                       # magnus-node (main binary)
│   ├── sidecar/                    # magnus-sidecar
│   ├── bench/                      # magnus-bench (benchmarking)
│   └── indexer/                    # Block/event indexer
│
├── crates/
│   ├── core/                       # Core primitives
│   │   ├── primitives/             # Basic types (Address, Hash, etc.)
│   │   ├── types/                  # Domain types (Block, Transaction)
│   │   ├── chainspec/              # Chain specification/genesis
│   │   └── error/                  # Error types (magnus-eyre)
│   │
│   ├── consensus/                  # Consensus layer (abstracted)
│   │   ├── engine/                 # ConsensusEngine trait + backend
│   │   ├── validator/              # Validator management
│   │   └── dkg/                    # DKG artifacts/ceremony
│   │
│   ├── execution/                  # Execution layer (abstracted)
│   │   ├── evm/                    # MagnusEvmConfig + ExecutionEngine backend
│   │   └── vm/                     # EVM integration (revm wrapper)
│   │
│   ├── storage/                    # Storage layer (abstracted)
│   │   ├── backend/                # StorageBackend trait + impl
│   │   ├── merkle/                 # Merkle tree implementation
│   │   └── cache/                  # Caching layer
│   │
│   ├── network/                    # Network layer
│   │   ├── transport/              # P2P transport
│   │   ├── rpc/                    # JSON-RPC server
│   │   └── sync/                   # State sync
│   │
│   ├── payment/                    # Payment-specific (Magnus core)
│   │   ├── token/                  # MIP-20 token standard
│   │   ├── policy/                 # MIP-403 transfer policies
│   │   ├── fee/                    # Fee manager
│   │   ├── amm/                    # Fee AMM (stablecoin conversion)
│   │   ├── memo/                   # ISO 20022 memo extensions
│   │   ├── batch/                  # Batch payment processing
│   │   ├── sponsor/                # Gas sponsorship
│   │   ├── currency/               # Multi-currency support
│   │   └── oracle/                 # Price oracle integration
│   │
│   ├── precompile/                 # Precompile infrastructure
│   │   ├── registry/               # Precompile registry
│   │   ├── macros/                 # Proc macros for precompiles
│   │   └── contracts/              # Built-in contract interfaces
│   │
│   ├── node/                       # Node components
│   │   ├── builder/                # Node builder pattern
│   │   ├── config/                 # Configuration management
│   │   ├── payload/                # Payload building
│   │   └── mempool/                # Transaction pool
│   │
│   └── sdk/                        # SDK components
│       ├── client/                 # RPC client
│       ├── provider/               # Provider abstraction
│       └── testing/                # Test utilities
│
├── contracts/                      # Solidity contracts
│   ├── src/
│   │   ├── MIP20.sol               # MIP-20 token standard
│   │   ├── MIP403Registry.sol      # Transfer policy registry
│   │   ├── FeeManager.sol          # Fee management
│   │   ├── FeeAMM.sol              # Fee AMM
│   │   ├── StablecoinDEX.sol       # Stablecoin DEX
│   │   └── interfaces/             # Contract interfaces
│   └── test/                       # Foundry tests
│
├── sdk/                            # External SDKs
│   ├── typescript/                 # TypeScript/JavaScript SDK
│   └── python/                     # Python SDK
│
├── tools/                          # Developer tools
│   ├── magnusup/                   # Installer (like rustup)
│   ├── genesis/                    # Genesis file generator
│   └── xtask/                      # Build automation
│
├── docs/                           # Documentation
│   ├── architecture/               # Architecture docs
│   ├── specs/                      # Technical specifications
│   └── guides/                     # Developer guides
│
└── e2e/                            # End-to-end tests
```

---

## 3. Abstraction Layer Design

### 3.1 Core Principle

**Public interfaces expose traits; implementations are private.**

```
┌─────────────────────────────────────────────────────────────┐
│                    Public API Layer                          │
├─────────────────────────────────────────────────────────────┤
│  ExecutionEngine    ConsensusEngine    StorageBackend       │
│  (trait)            (trait)            (trait)              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Private Backend Layer                       │
├─────────────────────────────────────────────────────────────┤
│  backend::           backend::          backend::           │
│  ParallelExecutor    SimplexConsensus   QMDBStorage         │
│  (FAFO inside)       (Simplex inside)   (QMDB inside)       │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Execution Engine Abstraction

```rust
// crates/execution/evm/src/lib.rs

/// Public trait for execution engines
pub trait ExecutionEngine: Send + Sync {
    /// Execute a batch of transactions
    fn execute_batch(
        &self,
        txs: Vec<Transaction>,
        state: &dyn StateProvider,
    ) -> Result<ExecutionResult, ExecutionError>;

    /// Execute a single transaction
    fn execute_single(
        &self,
        tx: &Transaction,
        state: &dyn StateProvider,
    ) -> Result<TransactionReceipt, ExecutionError>;

    /// Validate transaction before execution
    fn validate(&self, tx: &Transaction) -> Result<(), ValidationError>;

    /// Get engine capabilities
    fn capabilities(&self) -> EngineCapabilities;
}

#[derive(Debug, Clone)]
pub struct EngineCapabilities {
    pub parallel_execution: bool,
    pub max_parallelism: usize,
    pub supports_speculation: bool,
}

// Private backend implementation
mod backend {
    use super::*;

    /// Parallel execution backend (FAFO-based)
    ///
    /// Implementation details are internal.
    /// See docs/architecture/execution.md for design rationale.
    pub(crate) struct ParallelExecutor {
        // ... FAFO internals hidden here
    }

    impl ExecutionEngine for ParallelExecutor {
        // ... implementation
    }
}

/// Create the default execution engine
pub fn create_execution_engine(config: &ExecutionConfig) -> Box<dyn ExecutionEngine> {
    Box::new(backend::ParallelExecutor::new(config))
}
```

### 3.3 Consensus Engine Abstraction

```rust
// crates/consensus/engine/src/lib.rs

/// Public trait for consensus engines
pub trait ConsensusEngine: Send + Sync {
    /// Propose a new block
    fn propose(&self, payload: Payload) -> Result<Proposal, ConsensusError>;

    /// Vote on a proposal
    fn vote(&self, proposal: &Proposal) -> Result<Vote, ConsensusError>;

    /// Finalize a block
    fn finalize(&self, block: &Block) -> Result<FinalizedBlock, ConsensusError>;

    /// Get current consensus state
    fn state(&self) -> ConsensusState;
}

mod backend {
    /// BFT consensus backend (Simplex-based)
    pub(crate) struct BftConsensus {
        // ... Simplex internals hidden here
    }
}
```

### 3.4 Storage Backend Abstraction

```rust
// crates/storage/backend/src/lib.rs

/// Public trait for storage backends
pub trait StorageBackend: Send + Sync {
    /// Get value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;

    /// Set value
    fn set(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;

    /// Delete value
    fn delete(&self, key: &[u8]) -> Result<(), StorageError>;

    /// Commit pending changes
    fn commit(&self) -> Result<Hash, StorageError>;

    /// Get merkle proof
    fn get_proof(&self, key: &[u8]) -> Result<MerkleProof, StorageError>;
}

mod backend {
    /// High-performance storage backend (QMDB-based)
    pub(crate) struct MerkleStorage {
        // ... QMDB internals hidden here
    }
}
```

---

## 4. Crate Dependency Graph

```
                                ┌─────────────┐
                                │    apps/    │
                                │    node     │
                                └──────┬──────┘
                                       │
                    ┌──────────────────┼──────────────────┐
                    │                  │                  │
              ┌─────▼─────┐     ┌──────▼──────┐    ┌─────▼─────┐
              │ consensus │     │  execution  │    │  network  │
              │  engine   │     │   engine    │    │    rpc    │
              └─────┬─────┘     └──────┬──────┘    └─────┬─────┘
                    │                  │                  │
                    │           ┌──────▼──────┐          │
                    │           │   payment   │          │
                    │           │   (token,   │          │
                    │           │  fee, amm)  │          │
                    │           └──────┬──────┘          │
                    │                  │                  │
              ┌─────▼─────┐     ┌──────▼──────┐    ┌─────▼─────┐
              │ consensus │     │  precompile │    │  storage  │
              │ validator │     │  registry   │    │  backend  │
              └─────┬─────┘     └──────┬──────┘    └─────┬─────┘
                    │                  │                  │
                    └──────────────────┼──────────────────┘
                                       │
                                ┌──────▼──────┐
                                │    core/    │
                                │ primitives  │
                                │   types     │
                                └─────────────┘
```

---

## 5. Migration Checklist

### Phase 1: Structure (Week 1-2)

- [ ] Create magnus/ folder structure
- [ ] Copy source files to new locations
- [ ] Update all `Cargo.toml` paths
- [ ] Rename crate names (`tempo-*` → `magnus-*`)
- [ ] Update workspace `Cargo.toml`

### Phase 2: Symbols (Week 3-4)

- [ ] Rename module names
- [ ] Rename struct/enum names
- [ ] Rename function names
- [ ] Rename constant names
- [ ] Update doc comments

### Phase 3: Standards (Week 5-6)

- [ ] Rename TIP-20 → MIP-20
- [ ] Rename TIP-403 → MIP-403
- [ ] Update precompile addresses (constants only)
- [ ] Update RPC namespaces

### Phase 4: Abstraction (Week 7-8)

- [ ] Create ExecutionEngine trait
- [ ] Create ConsensusEngine trait
- [ ] Create StorageBackend trait
- [ ] Move implementations to `backend/` modules

### Phase 5: Validation (Week 9-10)

- [ ] Run `cargo build`
- [ ] Run all tests
- [ ] Run `grep -r "tempo" --include="*.rs"` (should return 0)
- [ ] Run `grep -r "TIP" --include="*.rs"` (should return 0)
- [ ] Security audit of changes

---

## 6. Precompile Address Mapping

| Precompile | Address | Original Constant | New Constant |
|------------|---------|-------------------|--------------|
| Fee Manager | `0xfeec...` | `TIP_FEE_MANAGER_ADDRESS` | `MIP_FEE_MANAGER_ADDRESS` |
| Token Factory | `0x20Fc...` | `TIP20_FACTORY_ADDRESS` | `MIP20_FACTORY_ADDRESS` |
| Policy Registry | `0x403C...` | `TIP403_REGISTRY_ADDRESS` | `MIP403_REGISTRY_ADDRESS` |
| Stablecoin DEX | `0xdec0...` | `STABLECOIN_DEX_ADDRESS` | `STABLECOIN_DEX_ADDRESS` |
| Default Token | `0x20C0...` | `PATH_USD_ADDRESS` | `MAGNUS_USD_ADDRESS` |

**Note:** Actual addresses remain unchanged for compatibility; only constant names change.

---

## 7. Multi-Fiat Stablecoin Architecture

### 7.1 Supported Currency Zones

| Zone | Currency | Example Tokens |
|------|----------|----------------|
| USD | US Dollar | USDC, USDT, MAGNUS_USD |
| VND | Vietnamese Dong | VNST, VND_STABLE |
| EUR | Euro | EURC, EURS |

### 7.2 Fee Token Validation (Updated)

```rust
// crates/payment/fee/src/validation.rs

/// Allowed currency zones for fee payment
pub const ALLOWED_CURRENCIES: &[&str] = &["USD", "VND", "EUR"];

/// Validate if token can be used for fee payment
pub fn is_valid_fee_token(token: &MIP20Token) -> Result<bool, Error> {
    let currency = token.currency()?;
    Ok(ALLOWED_CURRENCIES.contains(&currency.as_str()))
}
```

### 7.3 Oracle Integration (Phase 2)

```rust
// crates/payment/oracle/src/lib.rs

/// Oracle router for cross-currency rates
pub trait OracleRouter: Send + Sync {
    /// Get exchange rate between currencies
    fn get_rate(
        &self,
        from: &str,
        to: &str,
    ) -> Result<ExchangeRate, OracleError>;

    /// Check if rate is fresh
    fn is_rate_valid(&self, rate: &ExchangeRate) -> bool;
}

pub struct ExchangeRate {
    pub from_currency: String,
    pub to_currency: String,
    pub numerator: U256,
    pub denominator: U256,
    pub updated_at: u64,
    pub valid_until: u64,
}
```

---

## 8. Build Configuration

### 8.1 Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    # Apps
    "apps/node",
    "apps/sidecar",
    "apps/bench",
    "apps/indexer",

    # Core
    "crates/core/primitives",
    "crates/core/types",
    "crates/core/chainspec",
    "crates/core/error",

    # Consensus
    "crates/consensus/engine",
    "crates/consensus/validator",
    "crates/consensus/dkg",

    # Execution
    "crates/execution/evm",
    "crates/execution/vm",

    # Storage
    "crates/storage/backend",
    "crates/storage/merkle",
    "crates/storage/cache",

    # Network
    "crates/network/transport",
    "crates/network/rpc",
    "crates/network/sync",

    # Payment
    "crates/payment/token",
    "crates/payment/policy",
    "crates/payment/fee",
    "crates/payment/amm",
    "crates/payment/memo",
    "crates/payment/batch",
    "crates/payment/sponsor",
    "crates/payment/currency",
    "crates/payment/oracle",

    # Precompile
    "crates/precompile/registry",
    "crates/precompile/macros",
    "crates/precompile/contracts",

    # Node
    "crates/node/builder",
    "crates/node/config",
    "crates/node/payload",
    "crates/node/mempool",

    # SDK
    "crates/sdk/client",
    "crates/sdk/provider",
    "crates/sdk/testing",

    # Tools
    "tools/xtask",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/magnus-chain/magnus"

[workspace.dependencies]
# Magnus internal crates
magnus-primitives = { path = "crates/core/primitives" }
magnus-types = { path = "crates/core/types" }
magnus-chainspec = { path = "crates/core/chainspec" }
magnus-error = { path = "crates/core/error" }
magnus-consensus-engine = { path = "crates/consensus/engine" }
magnus-evm = { path = "crates/execution/evm" }
magnus-storage-backend = { path = "crates/storage/backend" }
magnus-payment-token = { path = "crates/payment/token" }
magnus-payment-fee = { path = "crates/payment/fee" }
magnus-precompile-registry = { path = "crates/precompile/registry" }

# External dependencies (unchanged from Tempo)
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
revm = { version = "19", default-features = false }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tracing = "0.1"
```

---

## 9. Documentation Requirements

### 9.1 Files to Create

| File | Purpose |
|------|---------|
| `docs/architecture/overview.md` | High-level architecture |
| `docs/architecture/execution.md` | Execution engine design (mentions FAFO here) |
| `docs/architecture/consensus.md` | Consensus design (mentions Simplex here) |
| `docs/architecture/storage.md` | Storage design (mentions QMDB here) |
| `docs/specs/MIP-20.md` | MIP-20 token standard |
| `docs/specs/MIP-403.md` | Transfer policy registry |
| `docs/guides/getting-started.md` | Developer quickstart |
| `docs/guides/running-node.md` | Node operation guide |

### 9.2 Tech Details in Docs Only

```markdown
<!-- docs/architecture/execution.md -->

# Execution Engine

Magnus uses a high-performance parallel execution engine capable of
processing 500,000+ transactions per second.

## Implementation Details

The execution engine is based on FAFO (Fast Atomic Fee Operations),
a speculative parallel execution framework that...
```

**Note:** Technology names (FAFO, Simplex, QMDB) appear ONLY in documentation, never in code symbols.

---

## 10. Verification Commands

After migration, run these commands to verify complete renaming:

```bash
# Should return 0 matches (excluding docs/)
grep -r "tempo" --include="*.rs" magnus/crates magnus/apps
grep -r "Tempo" --include="*.rs" magnus/crates magnus/apps
grep -r "TEMPO" --include="*.rs" magnus/crates magnus/apps

# Should return 0 matches
grep -r "TIP20" --include="*.rs" magnus/crates magnus/apps
grep -r "TIP_" --include="*.rs" magnus/crates magnus/apps
grep -r "TIP-" --include="*.rs" magnus/crates magnus/apps

# Build verification
cd magnus && cargo build --release

# Test verification
cd magnus && cargo test --all
```

---

## Appendix A: Tempo → Magnus Crate Mapping

| Tempo Crate | Magnus Location | New Name |
|-------------|-----------------|----------|
| `tempo-primitives` | `crates/core/primitives` | `magnus-primitives` |
| `tempo-chainspec` | `crates/core/chainspec` | `magnus-chainspec` |
| `tempo-eyre` | `crates/core/error` | `magnus-error` |
| `tempo-node` | `apps/node` | `magnus-node` |
| `tempo-sidecar` | `apps/sidecar` | `magnus-sidecar` |
| `tempo-bench` | `apps/bench` | `magnus-bench` |
| `tempo-consensus` | `crates/consensus/engine` | `magnus-consensus-engine` |
| `tempo-revm` | `crates/execution/vm` | `magnus-vm` |
| `tempo-evm` | `crates/execution/evm` | `magnus-evm` |
| `tempo-precompiles` | `crates/precompile/registry` | `magnus-precompile-registry` |
| `tempo-precompiles-macros` | `crates/precompile/macros` | `magnus-precompile-macros` |
| `tempo-contracts` | `crates/precompile/contracts` | `magnus-contracts` |
| `tempo-payload-types` | `crates/node/payload` | `magnus-payload` |
| `tempo-payload-builder` | `crates/node/payload` | (merged) |
| `tempo-transaction-pool` | `crates/node/mempool` | `magnus-mempool` |
| `tempo-alloy` | `crates/sdk/provider` | `magnus-provider` |
| `tempo-telemetry-util` | `crates/core/primitives` | (merged) |
| `tempo-faucet` | `tools/faucet` | `magnus-faucet` |
| `tempo-e2e` | `e2e/` | `magnus-e2e` |
| `magnus-consensus-engine` (legacy) | `crates/consensus/consensus-engine` | `magnus-consensus-engine` |
| `magnus-consensus-engine-config` (legacy) | `crates/node/consensus-engine-config` | `magnus-consensus-engine-config` |
| `dkg-onchain-artifacts` | `crates/consensus/dkg` | `magnus-dkg` |

---

## Appendix B: File Count Estimate

| Category | Tempo | Magnus (Plan 5-A) |
|----------|-------|-------------------|
| Rust crates | 24 | 42 |
| Binary targets | 3 | 4 |
| Solidity contracts | ~10 | ~10 |
| Test files | ~50 | ~60 |
| Doc files | ~20 | ~30 |

---

*Document Version: 1.0*
*Last Updated: January 2026*
*Status: Ready for Implementation*
