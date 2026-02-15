# `magnus-consensus`

<a href="https://github.com/refcell/magnus/actions/workflows/ci.yml"><img src="https://github.com/refcell/magnus/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/magnus/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Consensus application layer for Magnus.

This crate provides the bridge between Commonware consensus and REVM execution,
using trait-abstracted components for modularity.

## Key Types

- `ConsensusApplication` - Implements Commonware's Application trait
- `Block` - Commonware-compatible block type from `magnus-domain`
- `ExecutionOutcome` - Result of block execution

## Traits

All components are trait-abstracted for swappability:

- `Mempool` - Pending transaction pool
- `SnapshotStore` - Execution state caching
- `SeedTracker` - VRF seed management
- `BlockExecutor` - Transaction execution

## Architecture

```text
+--------------------------------------------------+
|              magnus-consensus                       |
|                                                   |
|  ConsensusApplication<M, S, SS, ST, E>           |
|       |         |        |       |       |       |
|       v         v        v       v       v       |
|   Mempool   StateDb  Snapshot  Seed   Block     |
|   trait     trait    Store    Tracker Executor  |
+--------------------------------------------------+
        |         |
        v         v
   magnus-traits  magnus-handlers
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
magnus-consensus = { path = "crates/node/consensus" }
```

## License

[MIT License](https://github.com/refcell/magnus/blob/main/LICENSE)
