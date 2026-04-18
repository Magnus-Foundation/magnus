<br>
<br>

<p align="center">
  <a href="https://magnus.xyz">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Magnus-Foundation/.github/refs/heads/main/assets/combomark-dark.svg">
      <img alt="magnus combomark" src="https://raw.githubusercontent.com/Magnus-Foundation/.github/refs/heads/main/assets/combomark-bright.svg" width="auto" height="120">
    </picture>
  </a>
</p>

<br>
<br>

# Magnus

The blockchain for payments at scale.

[Magnus](https://docs.magnus.xyz/) is a blockchain designed specifically for stablecoin payments. Its architecture focuses on high throughput, low cost, and features that financial institutions, payment service providers, and fintech platforms expect from modern payment infrastructure.

You can get started today by integrating with the [Magnus testnet](https://docs.magnus.xyz/quickstart/integrate-magnus), [building on Magnus](https://docs.magnus.xyz/guide/use-accounts), [running a Magnus node](https://docs.magnus.xyz/guide/node), reading the [Magnus protocol specs](https://docs.magnus.xyz/protocol) or by [building with Magnus SDKs](https://docs.magnus.xyz/sdk).

## What makes Magnus different

- [MIP‑20 token standard](https://docs.magnus.xyz/protocol/tip20/overview) (enshrined ERC‑20 extensions)

  - Predictable payment throughput via dedicated payment lanes reserved for MIP‑20 transfers (eliminates noisy‑neighbor contention).
  - Native reconciliation with on‑transfer memos and commitment patterns (hash/locator) for off‑chain PII and large data.
  - Built‑in compliance through [MIP‑403 Policy Registry](https://docs.magnus.xyz/protocol/tip403/overview): single policy shared across multiple tokens, updated once and enforced everywhere.

- Low, predictable fees in [stablecoins](https://docs.magnus.xyz/learn/stablecoins)

  - Users pay gas directly in USD-stablecoins at launch; the [Fee AMM](https://docs.magnus.xyz/protocol/fees/fee-amm#fee-amm-overview) automatically converts to the validator’s preferred stablecoin.
  - MIP‑20 transfers target sub‑millidollar costs (<$0.001).

- [Magnus Transactions](https://docs.magnus.xyz/guide/magnus-transaction) (native “smart accounts”)

  - Batched payments: atomic multi‑operation payouts (payroll, settlements, refunds).
  - Fee sponsorship: apps can pay users' gas to streamline onboarding and flows.
  - Scheduled payments: protocol‑level time windows for recurring and timed disbursements.
  - Modern authentication: passkeys via WebAuthn/P256 (biometric sign‑in, secure enclave, cross‑device sync).

- Performance and finality

  - Built on the [Reth SDK](https://github.com/paradigmxyz/reth), the most performant and flexible EVM (Ethereum Virtual Machine) execution client.
  - Simplex Consensus (via [Commonware](https://commonware.xyz/)): fast, sub‑second finality in normal conditions; graceful degradation under adverse networks.

- Coming soon

  - On‑chain FX and non‑USD stablecoin support for direct on‑chain liquidity; pay fees in more currencies.
  - Native private token standard: opt‑in privacy for balances/transfers coexisting with issuer compliance and auditability.

## What makes Magnus familiar

- Fully compatible with the Ethereum Virtual Machine (EVM), targeting the Osaka hardfork.
- Deploy and interact with smart contracts using the same tools, languages, and frameworks used on Ethereum, such as Solidity, Foundry, and Hardhat.
- All Ethereum JSON-RPC methods work out of the box.

While the execution environment mirrors Ethereum's, Magnus introduces some differences optimized for payments, described [here](https://docs.magnus.xyz/quickstart/evm-compatibility).

## Getting Started

### As a user

You can connect to Magnus's public testnet using the following details:

| Property           | Value                              |
| ------------------ | ---------------------------------- |
| **Network Name**   | Magnus Testnet (Moderato)           |
| **Currency**       | `USD`                              |
| **Chain ID**       | `42431`                            |
| **HTTP URL**       | `https://rpc.moderato.magnus.xyz`   |
| **WebSocket URL**  | `wss://rpc.moderato.magnus.xyz`     |
| **Block Explorer** | `https://explore.magnus.xyz`        |

Next, grab some stablecoins to test with from Magnus's [Faucet](https://docs.magnus.xyz/quickstart/faucet#faucet).

Alternatively, use [`cast`](https://github.com/foundry-rs/foundry):

```bash
cast rpc tempo_fundAddress <ADDRESS> --rpc-url https://rpc.moderato.magnus.xyz
```

### As an operator

We provide three different installation paths: installing a pre-built binary, building from source or using our provided Docker image.

- [Pre-built Binary](https://docs.magnus.xyz/guide/node/installation#pre-built-binary)
- [Build from Source](https://docs.magnus.xyz/guide/node/installation#build-from-source)
- [Docker](https://docs.magnus.xyz/guide/node/installation#docker)

See the [Magnus documentation](https://docs.magnus.xyz/guide/node) for instructions on how to install and run Magnus.

### As a developer

Magnus has several SDKs to help you get started building on Magnus:

- [TypeScript](https://docs.magnus.xyz/sdk/typescript)
- [Rust](https://docs.magnus.xyz/sdk/rust)
- [Go](https://docs.magnus.xyz/sdk/go)
- [Foundry](https://docs.magnus.xyz/sdk/foundry)

Want to contribute?

First, clone the repository:

```
git clone https://github.com/Magnus-Foundation/magnus
cd magnus
```

Next, install [`just`](https://github.com/casey/just?tab=readme-ov-file#packages).

Install the dependencies:

```bash
just
```

Build Magnus:

```bash
just build-all
```

Run the tests:

```bash
cargo nextest run
```

Start a `localnet`:

```bash
just localnet
```

## Contributing

Our contributor guidelines can be found in [`CONTRIBUTING.md`](https://github.com/Magnus-Foundation/magnus?tab=contributing-ov-file).

## Security

See [`SECURITY.md`](https://github.com/Magnus-Foundation/magnus?tab=security-ov-file). Note: Magnus is still undergoing audit and does not have an active bug bounty. Submissions will not be eligible for a bounty until audits have concluded.

## License

Licensed under either of [Apache License](./LICENSE-APACHE), Version
2.0 or [MIT License](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in these crates by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

## Heritage

Magnus is a hard fork of [Tempo](https://github.com/tempoxyz/tempo) at SHA
`786c8ce34`. The Tempo project — created by Stripe with consensus work by
Commonware — provided the foundation for consensus, EVM integration, and the
overall node architecture. Tempo and Stripe copyright lines remain in
[`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE) per the
MIT/Apache-2.0 attribution requirements. Magnus extends this foundation with
payments, gateway, and bridge subsystems described in the project's design
docs.
