# Magnus

**Magnus** is a high-performance EVM-compatible blockchain optimized for stablecoin payments, with a focus on the Vietnam market.

## Overview

Magnus is designed for:

- **Multi-stablecoin gas fees** - Pay transaction fees in USDC, USDT, VNST, or other approved stablecoins
- **High throughput** - 500,000+ TPS through parallel execution
- **Instant finality** - Sub-second transaction confirmation
- **Compliance-ready** - MIP-403 transfer policy registry for regulatory compliance
- **Payment-optimized** - ISO 20022 memo extensions, batch payments, and fee sponsorship

## Architecture

Magnus follows a layered architecture with abstraction boundaries:

```
┌─────────────────────────────────────────────────────────────┐
│                       Applications                           │
│              magnus-node, magnus-sidecar, etc.              │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────────┐
│                      Public Traits                           │
│   ExecutionEngine, ConsensusEngine, StorageBackend          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Private Backends                           │
│           (Implementation details hidden)                    │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
magnus/
├── apps/                   # Application binaries
│   ├── node/               # Main node binary
│   ├── sidecar/            # Sidecar services
│   └── bench/              # Benchmarking tools
│
├── crates/
│   ├── core/               # Core primitives and types
│   ├── consensus/          # Consensus layer
│   ├── execution/          # Execution layer (EVM)
│   ├── storage/            # Storage backends
│   ├── network/            # Networking
│   ├── payment/            # Payment-specific features
│   ├── precompile/         # Precompile contracts
│   ├── node/               # Node components
│   └── sdk/                # SDK libraries
│
├── contracts/              # Solidity contracts
├── tools/                  # Developer tools
├── docs/                   # Documentation
└── e2e/                    # End-to-end tests
```

## Key Standards

### MIP-20 (Token Standard)

MIP-20 is Magnus's native token standard, extending ERC-20 with:

- **Protocol-level gas fees** - `transferFeePreTx` and `transferFeePostTx` for atomic fee deduction
- **Transfer policies** - Integration with MIP-403 compliance registry
- **Memo support** - 32-byte memo field for payment references
- **Reward distribution** - Built-in staking reward mechanism

### MIP-403 (Transfer Policy Registry)

Compliance framework enabling:

- Sender/receiver whitelisting
- Transaction amount limits
- Velocity controls
- Geographic restrictions

## Quick Start

### Prerequisites

- Rust 1.85+
- Git

### Build

```bash
# Clone the repository
git clone https://github.com/magnus-chain/magnus
cd magnus

# Build release binary
cargo build --release

# Run tests
cargo test --all
```

### Run a Node

```bash
# Start a local development node
./target/release/magnus-node --dev

# Or with custom config
./target/release/magnus-node --config config.toml
```

## Documentation

- [Architecture Overview](docs/architecture/overview.md)
- [MIP-20 Specification](docs/specs/MIP-20.md)
- [MIP-403 Specification](docs/specs/MIP-403.md)
- [Migration Guide](docs/MIGRATION_GUIDE.md)
- [Plan 5-A Specification](docs/MAGNUS_V4_PLAN_5A_SPEC.md)

## Precompile Addresses

| Precompile | Address |
|------------|---------|
| Fee Manager | `0xfeec000000000000000000000000000000000000` |
| MIP-20 Factory | `0x20Fc000000000000000000000000000000000000` |
| MIP-403 Registry | `0x403C000000000000000000000000000000000000` |
| Stablecoin DEX | `0xdec0000000000000000000000000000000000000` |
| Default Token (MAGNUS_USD) | `0x20C0000000000000000000000000000000000000` |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

---

*Magnus - Powering the future of stablecoin payments*
