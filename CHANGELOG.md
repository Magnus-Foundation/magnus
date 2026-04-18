# Changelog

## Unreleased

### Renamed

- **Hard fork from Magnus to Magnus.** The project forked from
  [`Magnus-Foundation/magnus`](https://github.com/Magnus-Foundation/magnus) at SHA `786c8ce34`.
  All `magnus-*` crates renamed to `magnus-*`, binaries `magnus{,-bench,-sidecar}`
  renamed to `magnus{,-bench,-sidecar}`, Rust identifiers `Magnus*` and `MAGNUS_*`
  renamed accordingly. Improvement proposals moved from `tips/tip-*.md` to
  `mips/mip-*.md` with numbering preserved (including `mips/ref-impls/`
  Solidity reference implementations). Workspace restructured: crates grouped
  under `consensus/`, `evm/`, `node/`, `primitives/`, `util/`, `tools/` with
  new empty slots for `payments/`, `gateway/`, `bridge/`. No behavioral
  change; no chain ID change. RPC method namespaces (`#[rpc(namespace =
  "magnus")]`) intentionally kept as `"magnus"` for this refactor to avoid
  breaking existing API consumers; revisit with a chain-ID change in a
  follow-up spec. Consensus NAMESPACE constant also kept as `b"MAGNUS"` for
  the same reason. Magnus/Stripe copyright lines preserved verbatim in
  `LICENSE-MIT` and `LICENSE-APACHE`.
