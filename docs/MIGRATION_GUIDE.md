# Magnus Migration Guide: Tempo → Magnus

This guide provides step-by-step instructions for migrating from the Tempo codebase to Magnus.

---

## Prerequisites

- Rust 1.75+ (edition 2024)
- Git
- Basic understanding of the Tempo architecture

---

## Phase 1: Folder Structure Creation

### 1.1 Create Directory Tree

```bash
# Create all directories
mkdir -p magnus/{apps/{node,sidecar,bench,indexer},crates/{core/{primitives,types,chainspec,error},consensus/{engine,validator,dkg},execution/{engine,vm,state,context},storage/{backend,merkle,cache},network/{transport,rpc,sync},payment/{token,policy,fee,amm,memo,batch,sponsor,currency,oracle},precompile/{registry,macros,contracts},node/{builder,config,payload,mempool},sdk/{client,provider,testing}},contracts/{src/interfaces,test},sdk/{typescript,python},tools/{magnusup,genesis,xtask},docs/{architecture,specs,guides},e2e}
```

### 1.2 File Migration Map

| Source (Tempo) | Destination (Magnus) |
|----------------|---------------------|
| `bin/tempo/` | `apps/node/` |
| `bin/tempo-sidecar/` | `apps/sidecar/` |
| `bin/tempo-bench/` | `apps/bench/` |
| `crates/primitives/` | `crates/core/primitives/` |
| `crates/chainspec/` | `crates/core/chainspec/` |
| `crates/eyre/` | `crates/core/error/` |
| `crates/consensus/` | `crates/consensus/engine/` |
| `crates/revm/` | `crates/execution/vm/` |
| `crates/evm/` | `crates/execution/evm/` |
| `crates/precompiles/` | `crates/precompile/registry/` |
| `crates/precompiles-macros/` | `crates/precompile/macros/` |
| `crates/contracts/` | `crates/precompile/contracts/` |
| `crates/payload/` | `crates/node/payload/` |
| `crates/transaction-pool/` | `crates/node/mempool/` |
| `crates/node/` | `crates/node/builder/` |
| `crates/alloy/` | `crates/sdk/provider/` |
| `crates/e2e/` | `e2e/` |
| `crates/faucet/` | `tools/faucet/` |
| `crates/commonware-node/` | `crates/consensus/consensus-engine/` |
| `crates/commonware-node-config/` | `crates/node/consensus-engine-config/` |
| `crates/dkg-onchain-artifacts/` | `crates/consensus/dkg/` |
| `crates/telemetry-util/` | `crates/core/primitives/telemetry/` |
| `docs/specs/src/*.sol` | `contracts/src/` |
| `tempoup/` | `tools/magnusup/` |
| `xtask/` | `tools/xtask/` |

---

## Phase 2: Symbol Renaming

### 2.1 Automated Renaming Script

```bash
#!/bin/bash
# rename_symbols.sh - Run from magnus/ directory

# Case-sensitive replacements (order matters!)
find . -type f -name "*.rs" -exec sed -i '' \
    -e 's/TEMPO_TX_TYPE_ID/MAGNUS_TX_TYPE_ID/g' \
    -e 's/TEMPO_/MAGNUS_/g' \
    -e 's/TempoTransaction/MagnusTransaction/g' \
    -e 's/TempoHardfork/MagnusHardfork/g' \
    -e 's/TempoEvm/MagnusEvm/g' \
    -e 's/TempoResult/MagnusResult/g' \
    -e 's/TempoTx/MagnusTx/g' \
    -e 's/TempoError/MagnusError/g' \
    -e 's/TempoChainSpec/MagnusChainSpec/g' \
    -e 's/TempoUtilities/MagnusUtilities/g' \
    -e 's/Tempo/Magnus/g' \
    -e 's/tempo_/magnus_/g' \
    -e 's/tempo-/magnus-/g' \
    {} \;

# TIP → MIP renaming
find . -type f -name "*.rs" -exec sed -i '' \
    -e 's/TIP_FEE_MANAGER/MIP_FEE_MANAGER/g' \
    -e 's/TIP20_FACTORY/MIP20_FACTORY/g' \
    -e 's/TIP403_REGISTRY/MIP403_REGISTRY/g' \
    -e 's/TIP20Token/MIP20Token/g' \
    -e 's/TIP20Error/MIP20Error/g' \
    -e 's/TipFeeManager/MipFeeManager/g' \
    -e 's/TIP20/MIP20/g' \
    -e 's/TIP-20/MIP-20/g' \
    -e 's/TIP403/MIP403/g' \
    -e 's/TIP-403/MIP-403/g' \
    -e 's/is_tip20/is_mip20/g' \
    -e 's/tip20/mip20/g' \
    -e 's/tip_fee/mip_fee/g' \
    {} \;

# Solidity files
find . -type f -name "*.sol" -exec sed -i '' \
    -e 's/TIP20/MIP20/g' \
    -e 's/TIP_/MIP_/g' \
    -e 's/ITIP20/IMIP20/g' \
    {} \;

# Cargo.toml files
find . -type f -name "Cargo.toml" -exec sed -i '' \
    -e 's/tempo-/magnus-/g' \
    -e 's/name = "tempo"/name = "magnus"/g' \
    {} \;
```

### 2.2 Manual Review Required

After automated renaming, manually review:

1. **Doc comments** - Ensure they make sense after renaming
2. **String literals** - Some may need to keep original names (e.g., for compatibility)
3. **Test fixtures** - May reference external data with original names
4. **Configuration files** - JSON, TOML keys may need updating

---

## Phase 3: Cargo.toml Updates

### 3.1 Root Workspace

```toml
# magnus/Cargo.toml
[workspace]
resolver = "2"
members = [
    "apps/*",
    "crates/core/*",
    "crates/consensus/*",
    "crates/execution/*",
    "crates/storage/*",
    "crates/network/*",
    "crates/payment/*",
    "crates/precompile/*",
    "crates/node/*",
    "crates/sdk/*",
    "tools/xtask",
    "e2e",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/magnus-chain/magnus"
```

### 3.2 Individual Crate Example

```toml
# crates/core/primitives/Cargo.toml
[package]
name = "magnus-primitives"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
alloy-primitives = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
magnus-testing = { workspace = true }
```

---

## Phase 4: Abstraction Layer Implementation

### 4.1 Create Trait Files

```rust
// crates/execution/evm/src/traits.rs

use magnus_primitives::{Transaction, Hash};
use magnus_types::{ExecutionResult, TransactionReceipt};

/// Core execution engine trait
///
/// This trait defines the interface for transaction execution.
/// Implementations may use various strategies (sequential, parallel, speculative).
pub trait ExecutionEngine: Send + Sync + 'static {
    /// Execute a batch of transactions
    fn execute_batch(
        &self,
        transactions: &[Transaction],
        parent_hash: Hash,
    ) -> Result<ExecutionResult, ExecutionError>;

    /// Execute single transaction
    fn execute_one(
        &self,
        transaction: &Transaction,
        parent_hash: Hash,
    ) -> Result<TransactionReceipt, ExecutionError>;

    /// Get engine capabilities
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct EngineCapabilities {
    pub parallel: bool,
    pub speculative: bool,
    pub max_parallelism: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("State error: {0}")]
    StateError(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
```

### 4.2 Backend Module Pattern

```rust
// crates/execution/evm/src/lib.rs

mod traits;
mod backend;

pub use traits::*;

/// Create default execution engine
pub fn create_engine(config: ExecutionConfig) -> impl ExecutionEngine {
    backend::ParallelExecutor::new(config)
}

// crates/execution/evm/src/backend/mod.rs
mod parallel;

pub(crate) use parallel::ParallelExecutor;

// crates/execution/evm/src/backend/parallel.rs
use crate::traits::*;

/// Parallel execution engine implementation
///
/// Uses speculative execution for high throughput.
/// See docs/architecture/execution.md for details.
pub(crate) struct ParallelExecutor {
    config: ExecutionConfig,
    // ... internal fields
}

impl ExecutionEngine for ParallelExecutor {
    fn execute_batch(&self, txs: &[Transaction], parent: Hash) -> Result<ExecutionResult, ExecutionError> {
        // Implementation details hidden from public API
        todo!()
    }

    fn execute_one(&self, tx: &Transaction, parent: Hash) -> Result<TransactionReceipt, ExecutionError> {
        todo!()
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            parallel: true,
            speculative: true,
            max_parallelism: Some(self.config.max_threads),
        }
    }
}
```

---

## Phase 5: Verification

### 5.1 Build Check

```bash
cd magnus
cargo build --release 2>&1 | tee build.log

# Check for tempo references in errors
grep -i "tempo" build.log && echo "ERROR: tempo references found!" || echo "OK: No tempo references"
```

### 5.2 Symbol Audit

```bash
# Should all return 0 matches
echo "Checking for remaining tempo symbols..."

grep -rn "tempo" --include="*.rs" crates/ apps/ | grep -v "// tempo" | wc -l
grep -rn "Tempo" --include="*.rs" crates/ apps/ | wc -l
grep -rn "TEMPO" --include="*.rs" crates/ apps/ | wc -l
grep -rn "TIP20" --include="*.rs" crates/ apps/ | wc -l
grep -rn "TIP_" --include="*.rs" crates/ apps/ | wc -l

echo "Audit complete"
```

### 5.3 Test Suite

```bash
cd magnus
cargo test --all --release 2>&1 | tee test.log

# Verify all tests pass
grep "test result:" test.log
```

---

## Phase 6: Documentation Update

### 6.1 Required Doc Changes

| File | Changes |
|------|---------|
| `README.md` | Update project name, description |
| `docs/architecture/*.md` | Reference Magnus, mention underlying tech |
| `docs/specs/MIP-*.md` | Rename from TIP-*, update content |
| `CHANGELOG.md` | Add migration entry |

### 6.2 Code Comments

Search and update all doc comments:

```bash
# Find doc comments mentioning Tempo
grep -rn "/// .*[Tt]empo" --include="*.rs" crates/
grep -rn "//! .*[Tt]empo" --include="*.rs" crates/
```

---

## Troubleshooting

### Common Issues

#### 1. Circular Dependencies

If you encounter circular dependencies after restructuring:

```
error[E0391]: cycle detected when processing `magnus_core`
```

**Solution:** Review dependency graph in spec, ensure lower layers don't depend on higher layers.

#### 2. Missing Re-exports

If public types become inaccessible:

```
error[E0603]: module `types` is private
```

**Solution:** Add `pub use` statements in `lib.rs`:

```rust
pub use types::*;
```

#### 3. Feature Flag Mismatches

If features don't propagate correctly:

```
error: the feature `std` is not enabled
```

**Solution:** Ensure `Cargo.toml` propagates features:

```toml
[features]
default = ["std"]
std = ["magnus-primitives/std", "magnus-types/std"]
```

---

## Post-Migration Checklist

- [ ] All tests pass (`cargo test --all`)
- [ ] Build succeeds (`cargo build --release`)
- [ ] No tempo/TIP symbols in Rust code
- [ ] Documentation updated
- [ ] CI/CD pipelines updated
- [ ] Docker images renamed
- [ ] npm packages renamed (if applicable)

---

*Migration Guide Version: 1.0*
