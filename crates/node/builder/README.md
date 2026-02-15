# `magnus-builder`

<a href="https://github.com/refcell/magnus/actions/workflows/ci.yml"><img src="https://github.com/refcell/magnus/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/magnus/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Node builder for constructing Magnus nodes with consensus components.

This crate provides a builder pattern for assembling Magnus nodes with configurable
consensus providers and node components.

## Key Types

- `NodeBuilder` - main builder for constructing complete nodes
- `ConsensusProvider` - trait for pluggable consensus implementations
- `NodeComponents` - trait defining required node component interfaces
- `Random` - trait for randomness sources used in consensus

## Usage

```rust,ignore
use magnus_builder::{NodeBuilder, ConsensusProvider, NodeComponents};

// Build a node with custom components
let node = NodeBuilder::new()
    .with_consensus(consensus_provider)
    .with_components(components)
    .build()
    .await?;
```

## License

This project is licensed under the [MIT License](../../LICENSE).
