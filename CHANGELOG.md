# Changelog

## Unreleased

### Renamed

- **Hard fork from Tempo to Magnus.** The project forked from
  [`tempoxyz/tempo`](https://github.com/tempoxyz/tempo) at SHA `786c8ce34`.
  All `tempo-*` crates renamed to `magnus-*`, binaries `tempo{,-bench,-sidecar}`
  renamed to `magnus{,-bench,-sidecar}`, Rust identifiers `Tempo*` and `TEMPO_*`
  renamed accordingly. Improvement proposals moved from `tips/tip-*.md` to
  `mips/mip-*.md` with numbering preserved (including `mips/ref-impls/`
  Solidity reference implementations). Workspace restructured: crates grouped
  under `consensus/`, `evm/`, `node/`, `primitives/`, `util/`, `tools/` with
  new empty slots for `payments/`, `gateway/`, `bridge/`. No behavioral
  change; no chain ID change. RPC method namespaces (`#[rpc(namespace =
  "tempo")]`) intentionally kept as `"tempo"` for this refactor to avoid
  breaking existing API consumers; revisit with a chain-ID change in a
  follow-up spec. Consensus NAMESPACE constant also kept as `b"TEMPO"` for
  the same reason. Tempo/Stripe copyright lines preserved verbatim in
  `LICENSE-MIT` and `LICENSE-APACHE`.
