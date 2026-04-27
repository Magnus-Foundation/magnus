# Multi-Currency Fees — Engineering Implementation Plan

**Status:** Draft v1
**Date:** 2026-04-27
**Spec reference:** [`multi-currency-fees-design.md`](multi-currency-fees-design.md) v3.8.2
**Hardfork target:** T4 (next available after T3 which activates 2026-04-27 mainnet)
**Total effort estimate:** ~7 eng-weeks for one engineer / ~4 calendar weeks with 2-3 engineers in parallel

This plan breaks v3.8.2 into 8 sequenced deliverables. Each is a self-contained PR (or small PR cluster), independently testable, with a clear hardfork gate. Audit checkpoints are listed between groups; external audit happens after group 6 before any of this lands on mainnet.

---

## Sequencing summary

```
              ┌─ G1 (currency registry)
              ├─ G2 (validator accept-set)  ──┬──→ G3 (Fee AMM removal)
              ├─ G4 (IssuerRegistry precomp.) ─┤
              │                               ├──→ G5 (MagnusUSD genesis)
G0 (scaffold) ┤                               │
              ├─ G6 (disable-currency hybrid)─┤
              ├─ G7 (off-boarding + escrow) ──┤
              └─ G8 (fee-token inference) ────┘
                                              │
                                              ▼
                                         AUDIT
                                              │
                                              ▼
                                    Testnet (Moderato) deployment
                                              │
                                              ▼
                                       Mainnet (T4 activation)
```

**Critical-path groups (block mainnet):** G1, G2, G3, G4, G5, G8.

**Non-critical-path (ship anytime, can defer to T4.1):** G6, G7. These add operational tooling but neither is required for the core multi-currency fee flow at launch. Could ship in T4 if time permits, otherwise T4.1.

**Decision recommended:** ship G6 and G7 in T4 to avoid carrying technical debt into mainnet.

---

## G0 — Scaffold & shared types (~3 eng-days)

**Purpose:** establish the new precompile module structure and shared error/event types so subsequent groups can land independently.

### Deliverables

- New module: `crates/evm/precompiles/src/mip20_issuer_registry/` with `mod.rs`, `dispatch.rs` skeleton (no logic, just stubs).
- New ABI file: `crates/evm/contracts/src/precompiles/mip20_issuer_registry.rs` with `sol!` block defining `IMIP20IssuerRegistry` interface (storage struct, events, errors, function selectors). Logic empty.
- Add `MIP20_ISSUER_REGISTRY_ADDRESS = 0x20FA000000000000000000000000000000000000` to `crates/evm/contracts/src/precompiles/mod.rs`.
- Wire stub precompile into `extend_magnus_precompiles` at [`crates/evm/precompiles/src/lib.rs:114`](../crates/evm/precompiles/src/lib.rs#L114).
- New file `crates/evm/precompiles/src/mip_fee_manager/currency_registry.rs` (empty stub).
- New error variants in `MipFeeManager` and `MIP20Factory` ABIs (used by G1+):
  - `IssuerNotApproved(address, string)` — declared ONCE on `IMIP20IssuerRegistry`; factory bubbles up the registry's error rather than duplicating it (avoids selector collision)
  - `CurrencyNotRegistered(string)`
  - `CurrencyDisabled(string)`
  - `FeeTokenNotInferable`
  - `FeeTokenNotAccepted(address, address)`

### Acceptance criteria

- `cargo check --workspace` passes.
- New precompile address resolves at startup; calling it returns `MethodNotImplemented` (placeholder).
- Existing test suite still green.

### PR boundary

Single PR. Diff is mostly additive; no behavior change.

---

## G1 — Currency registry (~5 eng-days)

**Purpose:** add the on-chain currency allowlist that gates which fiat currencies are gas-eligible.

### Deliverables

- Storage in `MipFeeManager`:
  ```rust
  // currency_registry.rs
  pub struct CurrencyConfig {
      enabled: bool,
      added_at_block: u64,
      enabled_at_block: u64,
  }
  pub struct CurrencyRegistry {
      supported: Mapping<String, CurrencyConfig>,
  }
  ```
- New `validate_supported_currency(token: Address) -> Result<()>` helper in `crates/evm/precompiles/src/mip20/mod.rs`.
- Governance functions in `MipFeeManager`:
  - `addCurrency(string code, ...)` — EIP-712 multisig sig
  - `enableCurrency(string code, ...)` — EIP-712 multisig sig
- Read-only views: `getCurrencyConfig(string)`, `isCurrencyEnabled(string)`.
- Events: `CurrencyAdded`, `CurrencyEnabled`.
- Genesis state: `supportedCurrencies["USD"] = {enabled: true, addedAtBlock: 0, enabledAtBlock: 0}` on testnet; `{USD, VND}` on mainnet.

### Tests

- Unit: governance happy path, replay protection (nonce reuse), expiry rejection.
- Unit: `validate_supported_currency` accepts USD when enabled, rejects unregistered currency, rejects disabled currency.
- Unit: enable-when-already-enabled is no-op or revert (decide; recommend revert for clarity).
- Integration: testnet genesis JSON contains `{USD: enabled}` correctly parseable.

### Acceptance criteria

- `cargo test -p magnus-precompiles --test currency_registry` green.
- Foundry harness (when added in G4) can call `addCurrency("VND")` from multisig and verify state change.

### PR boundary

Single PR. ~600-800 LoC including tests.

### Dependencies

G0 (errors and ABI scaffolding).

---

## G2 — Validator multi-token accept-set (~7 eng-days)

**Purpose:** replace single-token validator preference with multi-token accept-set. Removes per-user fee-token state. Removes `DEFAULT_FEE_TOKEN` fallback.

### Deliverables

- Storage migration in `MipFeeManager`:
  ```rust
  // OLD:
  validator_tokens: Mapping<Address, Address>,
  user_tokens:      Mapping<Address, Address>,

  // NEW:
  validator_accepted_tokens: Mapping<Address, Mapping<Address, bool>>,
  validator_token_list:      Mapping<Address, Vec<Address>>,
  // user_tokens: REMOVED entirely
  ```
- Replace `set_validator_token` with `add_accepted_token(token)` and `remove_accepted_token(token)`.
- Replace `get_validator_token` with `accepts_token(validator, token)` and `get_accepted_tokens(validator)`.
- Add `is_accepted_by_any_validator(token)` view.
- Remove `set_user_token`, `userTokens`, `get_user_token` entirely.
- Remove `DEFAULT_FEE_TOKEN` fallback from `get_validator_token` semantics. New behavior: if validator has empty accept-set, fee collection reverts with `ValidatorAcceptSetEmpty`.
- Add `MAX_ACCEPT_SET_SIZE = 32` constant; enforce in `add_accepted_token`.
- Update `IFeeManager` ABI to drop `setUserToken`, `userTokens`, `setValidatorToken`, `validatorTokens` events/errors; add new functions.

### Tests

- Unit: `add_accepted_token` validates `is_tip20` + `validate_supported_currency`; rejects same-block-as-beneficiary.
- Unit: `MAX_ACCEPT_SET_SIZE` enforcement (33rd add reverts).
- Unit: `remove_accepted_token` removes from both map and list, no orphans.
- Unit: empty accept-set + fee collection → `ValidatorAcceptSetEmpty` revert.
- Integration: validator with `{USDC, USDT, vndBank}` correctly reports membership for all three.
- Migration test: existing genesis JSON without `validator_tokens` populated produces empty accept-sets.

### Acceptance criteria

- `cargo test -p magnus-precompiles` green.
- Storage layout change is documented; existing data migration handled (storage slots renumbered).

### PR boundary

Single PR. Larger LoC (~1500) due to scope of FeeManager changes. Probably worth splitting into two PRs:
- G2a: storage + add/remove/views (no fee-path changes yet)
- G2b: integrate into fee path (calls `accepts_token` instead of `validator_tokens`)

### Dependencies

G1 (currency registry — `validate_supported_currency` is called by `add_accepted_token`).

### Risks

- **Storage migration** is the primary concern. If this hits a chain with existing `validator_tokens` state, that state becomes orphaned. For testnet: re-genesis OK. For mainnet: this is a breaking change baked into T4 hardfork; existing data is reset. **Document explicitly.**

---

## G3 — Fee AMM removal (~4 eng-days, REVISED — must be hardfork-gated)

**Important revision noted 2026-04-28 during scoping:** the original plan said "delete `amm.rs` entirely." That is unsafe in isolation — pre-T4 blocks committed to Moderato testnet (and any future devnet) execute the AMM swap path. A node binary without AMM code cannot replay those blocks, breaking sync from genesis.

**Real G3 must be split:**

1. **G3a — Add T4-gated direct-credit path.** Introduce `settle_fee` (direct-credit-or-revert per the spec) and a `cfg.spec.is_t4()` branch in `collect_fee_post_tx`. Pre-T4 keeps the AMM path. Post-T4 uses `settle_fee`. AMM code stays in tree.
2. **G3b — Delete AMM code.** Only after T4 has fully cycled out of replay-from-genesis needs (typically requires a network re-genesis or a pruning checkpoint). For Moderato testnet this is operationally a re-genesis at T4. For mainnet it's the launch state.

The original "delete amm.rs entirely" deliverable belongs in G3b, not G3a. G3a is the safe incremental step.

**Same hardfork-gating concern applies to G2b** (legacy `user_tokens` / `validator_tokens` removal) — `revm/handler.rs` reads `user_tokens` to determine the user's fee token. Removing the storage breaks pre-T4 sync. G2b must add the new path under T4 gating; full removal waits for re-genesis.



**Purpose:** delete the Fee AMM entirely. Replace `swap_fee` AMM-call path with direct-credit-or-revert.

### Deliverables

- Delete `crates/evm/precompiles/src/mip_fee_manager/amm.rs` entirely.
- Delete `ITIPFeeAMM` interface from `crates/evm/contracts/src/precompiles/mip_fee_manager.rs`.
- Replace `swap_fee` flow in `MipFeeManager`:
  ```rust
  // OLD: AMM swap when userToken != validatorToken
  // NEW: revert with FeeTokenNotAccepted if validator doesn't accept user's token
  fn settle_fee(validator: Address, fee_token: Address, amount: u128) -> Result<()> {
      if !self.accepts_token(validator, fee_token)? {
          return Err(FeeManagerError::fee_token_not_accepted(validator, fee_token).into());
      }
      self.increment_collected_fees(validator, fee_token, amount)?;
      Ok(())
  }
  ```
- Free up storage slots 0-3 in `MipFeeManager` (previously held AMM pool data).
- Update tests: remove all AMM-related test cases.

### Tests

- Unit: same-token credit (Case A from §5.1) works.
- Unit: same-currency-different-token now reverts (was AMM swap).
- Unit: cross-currency reverts (same as before, different code path).
- Gas regression: fee collection now ≤ 8k gas (was ~13k with AMM).

### Acceptance criteria

- `cargo test -p magnus-precompiles` green.
- No code paths reference `TIPFeeAMM` or `Pool` types after this PR.
- `mip_fee_manager/amm.rs` is fully deleted.

### PR boundary

Single PR. Mostly deletion (~3000 LoC removed, ~200 added).

### Dependencies

G2 (uses `accepts_token` view from G2).

### Risks

- **Storage slot 0-3 are reclaimed but must not be reused for unrelated state** until storage layout is documented. Reserve them for future use; document in storage layout MIP.

---

## G4 — IssuerRegistry precompile (~7 eng-days)

**Purpose:** implement the per-currency issuer allowlist gate. Factory rejects unauthorized deployments.

### Deliverables

- Implement `crates/evm/precompiles/src/mip20_issuer_registry/mod.rs`:
  - Storage: `approved_issuers: Mapping<String, Mapping<Address, bool>>`, `approved_issuer_list: Mapping<String, Vec<Address>>`.
  - Functions: `is_approved_issuer`, `get_approved_issuers`, `add_approved_issuer` (gov), `remove_approved_issuer` (gov).
  - Events: `IssuerApproved`, `IssuerRevoked`.
  - EIP-712 typehashes: `ADD_APPROVED_ISSUER_TYPEHASH`, `REMOVE_APPROVED_ISSUER_TYPEHASH`.
  - Per-digest nonce replay protection (`_nonceUsed`).
  - Domain separator: `(name="MIP20IssuerRegistry", version="1", chainId, address)`.
- Implement `dispatch.rs` for ABI routing.
- Wire registry check into `MIP20Factory.create_token`:
  ```rust
  // In mip20_factory/mod.rs at the top of create_token:
  let registry = MIP20IssuerRegistry::new();
  if !registry.is_approved_issuer(&call.currency, sender)? {
      return Err(MIP20IssuerRegistryError::issuer_not_approved(sender, call.currency.clone()).into());
  }
  ```
- Factory bubbles up the IssuerRegistry's `IssuerNotApproved` error directly. **Do NOT** declare a duplicate `IssuerNotApproved(address,string)` on `IMIP20Factory` — identical Solidity error definitions produce the same 4-byte selector, ambiguating the global decoder registry. The G0 implementation already enforces this.
- Reserved-address path (`create_token_reserved_address`) does NOT check the registry — bypass is intentional and documented.

### Tests

- Unit: governance happy path; multisig signature verification.
- Unit: replay protection via nonce.
- Unit: per-currency scope — adding Tether for USD does not authorize Tether for VND.
- Unit: factory rejects unapproved deploy with `IssuerNotApproved`.
- Unit: factory accepts approved deploy.
- Unit: reserved-address path bypasses registry (deploys MagnusUSD even when registry is empty).
- Integration: empty registry at genesis; first `addApprovedIssuer` via multisig works.
- Adversarial: factory gate cannot be bypassed by clever calldata, struct manipulation, etc.

### Acceptance criteria

- `cargo test -p magnus-precompiles --test issuer_registry` green.
- New precompile responds at `0x20FA000000000000000000000000000000000000`.
- Factory gate prevents unauthorized deployment in all permutations.

### PR boundary

Single PR. ~1500 LoC including tests.

### Dependencies

G0 (scaffold and address constant), G1 (currency registry — `addApprovedIssuer` checks `isCurrencyEnabled`).

---

## G5 — MagnusUSD genesis deployment (~3 eng-days)

**Purpose:** replace inherited Tempo `pathUSD` genesis bytecode at `0x20C0…0000` with Magnus Foundation-issued `MagnusUSD` at `0x20C0…0010`.

### Deliverables

- Update `crates/evm/contracts/src/precompiles/mod.rs`:
  ```rust
  // OLD: pub const PATH_USD_ADDRESS = address!("0x20C0...0000");
  // NEW:
  pub const MAGNUS_USD_ADDRESS: Address = address!("0x20C0000000000000000000000000000000000010");
  ```
  Search-and-replace `PATH_USD_ADDRESS` → `MAGNUS_USD_ADDRESS` across the codebase (renames + value change).
- Update `mip20_factory/mod.rs` slot-0 special-case ([line 197](../crates/evm/precompiles/src/mip20_factory/mod.rs#L197)): generalize from `address == PATH_USD_ADDRESS` to first-of-currency rule per spec §4.4a.
- Genesis JSON updates:
  - `crates/primitives/chainspec/src/genesis/dev.json`
  - `crates/primitives/chainspec/src/genesis/moderato.json`
  - `crates/primitives/chainspec/src/genesis/presto.json`
  - **Remove** existing allocation at `0x20c0000000000000000000000000000000000000`.
  - **Add** new allocation at `0x20c0000000000000000000000000000000000010`:
    - MIP-20 bytecode (existing)
    - Storage initialization for: `name = "MagnusUSD"`, `symbol = "mUSD"`, `currency = "USD"`, `quoteToken = 0x0`, `admin = magnusFoundationMultisig`, `decimals = 6`.
- Update genesis-init code that references slot-0 of the reserved range to use the new address.

### Tests

- Unit: genesis loader correctly populates MagnusUSD at new address.
- Unit: `is_tip20(0x20C0…0000)` returns false (slot is empty).
- Unit: `is_tip20(0x20C0…0010)` returns true (deployed).
- Unit: `MIP20Token::from_address(MAGNUS_USD_ADDRESS).currency()` returns "USD".
- Integration: testnet (Moderato) launches with MagnusUSD at correct address; can be referenced as `quoteToken` for subsequent factory deploys.
- Migration: confirm no code paths still reference `0x20C0…0000` after the migration.

### Acceptance criteria

- `cargo test --workspace` green.
- All three genesis files are consistent.
- Block explorer shows MagnusUSD at `0x20C0…0010`, slot `0x20C0…0000` is empty.

### PR boundary

Single PR. ~400 LoC of code + significant genesis JSON changes.

### Dependencies

G2 (replaces existing path that referenced PATH_USD_ADDRESS).

### Risks

- **Genesis JSON state has many bytes** (the existing pathUSD allocation is large bytecode). Easy to introduce typos. Recommend generating the new genesis allocation programmatically (small script) rather than hand-editing JSON.

---

## G6 — Disable-currency hybrid mechanism (~6 eng-days)

**Purpose:** implement standard deprecation + emergency disable + prune mechanisms per spec §11.1.

### Deliverables

- Extend `CurrencyConfig` (storage migration in G1 territory):
  ```rust
  pub struct CurrencyConfig {
      enabled: bool,
      deprecating: bool,                      // NEW
      deprecation_activates_at: u64,          // NEW
      added_at_block: u64,
      enabled_at_block: u64,
      last_pruned_at_block: u64,              // NEW
  }
  ```
- Add global config storage:
  ```rust
  deprecation_grace_period: u64,              // genesis: 30 days
  emergency_disable_threshold: u8,            // genesis: 7
  ```
- New functions:
  - `disable_currency(string code, ...)` — standard governance
  - `emergency_disable_currency(string code, ...)` — emergency threshold
  - `prune_currency(string code, uint256 maxIterations)` — anyone, paginated
  - `set_deprecation_grace_period(uint64, ...)` — standard governance
  - `set_emergency_disable_threshold(uint8, ...)` — current emergency threshold (NOT standard)
- Sanity bounds on setters: grace period in `[1 hour, 365 days]`, threshold in `(5, 9]`.
- Behavior modifications:
  - Factory `createToken` reverts with `CurrencyDeprecating` if currency is in grace period.
  - `add_accepted_token` reverts with `CurrencyDeprecating` if currency is in grace period.
  - `settle_fee` checks `enabled` flag. Emits `CurrencyDeprecationWarning` event during grace period.
- New events: `CurrencyDisabling`, `CurrencyDisabledEmergency`, `CurrencyDisabled`, `CurrencyDeprecationWarning`, `CurrencyPruned`, `DeprecationGracePeriodChanged`, `EmergencyDisableThresholdChanged`.
- New errors: `CurrencyDeprecating`, `CurrencyDisabledEmergency`, `EmergencyThresholdRequired`.

### Tests

- Unit: `disableCurrency` triggers grace period; existing fees still work; new factory deploys + accept-token blocked.
- Unit: `emergencyDisableCurrency` requires emergency threshold; rejects standard threshold sigs.
- Unit: setting emergency threshold requires CURRENT emergency threshold (cannot use standard).
- Unit: grace expiry transitions from `enabled` to `!enabled` automatically (lazy check on next tx).
- Unit: `pruneCurrency` removes tokens of disabled currency from validator accept-sets, paginated.
- Unit: sanity bounds reject out-of-range values.
- Adversarial: 5-of-9 signers cannot lower emergency threshold to themselves.
- Integration: full flow — register → enable → disable → grace → expiry → prune → re-enable.

### Acceptance criteria

- `cargo test -p magnus-precompiles --test disable_currency` green.
- All behavior matrix entries from spec §11.1 pass.

### PR boundary

Single PR. ~1200 LoC including tests.

### Dependencies

G1 (extends `CurrencyConfig`), G2 (modifies `add_accepted_token` and `settle_fee` paths).

---

## G7 — Validator off-boarding + escrow (~6 eng-days)

**Purpose:** implement the off-boarding flow with bounded-gas safe-transfer fallback to escrow.

### Deliverables

- Storage:
  ```rust
  foundation_escrow_address: Address,
  escrowed_fees: Mapping<Address, Mapping<Address, u128>>,
  escrow_claims: Mapping<Address, ClaimRecord>,
  escrow_claim_window: u64,                   // genesis: 365 days
  ```
- New functions:
  - `offboardValidator(address validator, ...)` — standard governance
  - `claimEscrowedFees(address validator, address token, address recipient, ...)` — validator's signature
  - `sweepExpiredEscrow(address validator, address token, ...)` — standard governance
  - `setEscrowClaimWindow(uint64, ...)` — standard governance
  - `setFoundationEscrowAddress(address, ...)` — standard governance
- Internal helper `_try_safe_transfer` per spec §11.2:
  ```rust
  fn _try_safe_transfer(token: Address, to: Address, amount: u128) -> bool {
      // bounded-gas low-level call (100k gas cap)
      // handles USDT-style void return
      // returns false on any failure
  }
  ```
- Sanity bounds: `escrow_claim_window` in `[30 days, 1825 days]`.
- New events: `ValidatorOffboarded`, `FeesOffboardDelivered`, `FeesOffboardEscrowed`, `EscrowedFeesClaimed`, `EscrowSwept`, `EscrowClaimWindowChanged`, `FoundationEscrowAddressChanged`.
- New errors: `ValidatorNotOffboarded`, `ClaimWindowExpired`, `ClaimWindowActive`, `InvalidValidatorSig`, `NoEscrowedFees`.
- Genesis: `foundation_escrow_address = magnusFoundationMultisig`, `escrow_claim_window = 365 days`.

### Tests

- Unit: off-board with working address → direct delivery; assert `FeesOffboardDelivered` event.
- Unit: off-board with reverting recipient → escrow path; assert `FeesOffboardEscrowed` event.
- Unit: off-board with USDT-style void-return token → direct delivery succeeds (data.length == 0 path).
- Unit: claim within window → success.
- Unit: claim after window → `ClaimWindowExpired`.
- Unit: sweep before window → `ClaimWindowActive`.
- Unit: sweep after window → success, funds → escrow address.
- Adversarial: malicious recipient with gas-bomb hook → `_try_safe_transfer` bounded gas prevents OOG.
- Adversarial: signature replay across (validator, token) pairs.
- Integration: full flow — off-board → some delivered, some escrowed → validator claims partial → window expires → foundation sweeps remainder.

### Acceptance criteria

- `cargo test -p magnus-precompiles --test offboarding` green.
- `_try_safe_transfer` exhibits expected behavior across MIP-20, USDT-pattern (void return), reverting tokens, blocklisted recipients.

### PR boundary

Single PR. ~1400 LoC including tests.

### Dependencies

G2 (uses `validator_token_list` to iterate tokens).

### Risks

- **Bounded-gas low-level call** is historically fraught in Solidity. Need careful review by security-minded engineer or auditor. Reference Compound, Aave for similar patterns.
- Recommend additional fuzzer focus for `_try_safe_transfer` against malicious tokens.

---

## G8 — Protocol-side fee-token inference + router selector registry (~5 eng-days)

**Purpose:** implement `infer_fee_token` and the governance-managed router selector registry per spec §11.3.

### Deliverables

- Storage in `MipFeeManager`:
  ```rust
  struct RouterDescriptor {
      selector: [u8; 4],
      token_input_arg_index: u8,
      registered: bool,
  }
  router_selectors: Mapping<Address, Mapping<[u8; 4], RouterDescriptor>>,
  ```
- New functions:
  - `register_router_selector(address router, bytes4 selector, uint8 argIndex, ...)` — governance
  - `unregister_router_selector(address router, bytes4 selector, ...)` — governance
- Internal helper `infer_fee_token(tx)` invoked during `collect_fee_pre_tx`:
  ```rust
  fn infer_fee_token(tx: &TxEnvelope) -> Result<Address> {
      if let Some(token) = tx.fee_token {
          return Ok(token);
      }
      let to = tx.to.ok_or(FeeManagerError::FeeTokenNotInferable)?;
      if to.is_tip20() {
          return Ok(to);
      }
      if let Some(token) = parse_known_router_calldata(to, &tx.input)? {
          return Ok(token);
      }
      Err(FeeManagerError::FeeTokenNotInferable.into())
  }
  ```
- Calldata parsing helper that decodes the registered argument index from the tx input.
- Genesis pre-registration of known routers:
  - Stablecoin DEX (`0xdec0…`): `swapExactAmountIn` and `swapExactAmountOut` selectors at arg index 0.
- New events: `RouterSelectorRegistered`, `RouterSelectorUnregistered`.
- New errors: `FeeTokenNotInferable` (already added in G0), `RouterSelectorNotFound`, `CalldataDecodeFailed`.

### Tests

- Unit: explicit `feeToken` → respected.
- Unit: direct MIP-20 transfer → token = `tx.to`.
- Unit: registered DEX swap → token = decoded from arg index 0.
- Unit: unregistered router call → `FeeTokenNotInferable`.
- Unit: malformed calldata for registered router → `CalldataDecodeFailed`.
- Unit: registering an existing selector (re-register) — decide: revert or update; recommend update with warning event.
- Integration: end-to-end tx flow — wallet submits MIP-20 transfer with no `feeToken`; protocol infers; fee collected correctly.
- Integration: unregistered DApp call submitted with no `feeToken` → reverts with clear error.

### Acceptance criteria

- `cargo test -p magnus-precompiles --test fee_token_inference` green.
- Genesis-pre-registered routers verified in genesis JSON.

### PR boundary

Single PR. ~1000 LoC including tests.

### Dependencies

G0 (errors), G1 (currency registry — inference doesn't require currency check, but consistent governance pattern), G2 (uses `accepts_token` after inference).

---

## Audit checkpoint

**After G6 completes (groups G0-G6 merged), but before G7-G8 land:**

- External audit engagement (Trail of Bits, Zellic, OpenZeppelin, or equivalent).
- Audit scope: all changes in G0-G6.
- Critical-path items audit MUST flag:
  - IssuerRegistry governance signature verification + replay protection
  - Storage layout migration safety
  - Disable-currency state machine (no way to bypass `enabled` check)
  - MagnusUSD genesis deployment integrity
- Estimated audit duration: 2-3 weeks. Remediation period: 1-2 weeks.
- After audit: G7-G8 ship in a follow-up PR cycle (also audited but lighter touch since they're additive).

**Total elapsed time including audit:** ~10-12 weeks from start of G0 to T4 mainnet activation.

---

## Testnet (Moderato) deployment

**After all 8 groups complete + audit findings closed:**

1. Branch `magnus-multi-currency-fees-v1` is rebased on `main`.
2. Deploy to Moderato (or its v3.5 §10 successor with new chain ID, per separate decision).
3. Initial state per spec §10 §10.1 launch sequence.
4. Soak test for 14 days minimum:
   - Magnus Foundation deploys MagnusUSD genesis.
   - Foundation calls `addApprovedIssuer("USD", magnusFoundationAddr)`.
   - At least one external test issuer (e.g. mock Tether) onboards via the gov flow.
   - Validators populate accept-sets.
   - Synthetic load: 10k+ transactions across multiple tokens / currencies / validators.
   - Reconciliation invariant per spec §11 holds (sum of fees collected = sum of fees paid by users).
5. Public testnet bug bounty.

---

## Mainnet (T4 hardfork) deployment

**After successful 14-day testnet soak + bug bounty close:**

1. T4 activation timestamp set in `crates/primitives/chainspec/src/constants.rs` (~6-week target per design doc §10).
2. Pre-launch coordination:
   - Magnus Foundation USD reserves verified at banking partner (regulatory prerequisite).
   - VN partner bank reserves verified for vndBank.
   - Multisig signers physically available; signed payloads pre-prepared.
3. T4 activates at the configured timestamp.
4. Within minutes of activation:
   - Foundation multisig calls `addApprovedIssuer("USD", magnusFoundationAddr)` and `addApprovedIssuer("VND", vnPartnerBankAddr)`.
   - Foundation deploys MagnusUSD initial supply (mint against reserves).
   - VN bank deploys vndBank initial supply.
   - Foundation validators call `addAcceptedToken(MagnusUSD)` and `addAcceptedToken(vndBank)`.
5. First fee transactions succeed.
6. Monitoring: dashboards verify `collectedFees` accumulating correctly per validator-org.

---

## Effort and timeline summary

| Group | Effort | Calendar (1 eng) | Calendar (3 engs parallel) |
|---|---|---|---|
| G0 — Scaffold | 3 ed | 1 wk | 0.5 wk |
| G1 — Currency registry | 5 ed | 1 wk | 0.5 wk (parallel with G2-G4) |
| G2 — Validator accept-set | 7 ed | 1.5 wk | 1 wk |
| G3 — Fee AMM removal | 4 ed | 1 wk | 0.5 wk (after G2) |
| G4 — IssuerRegistry | 7 ed | 1.5 wk | 1 wk (parallel with G2) |
| G5 — MagnusUSD genesis | 3 ed | 0.5 wk | 0.5 wk (after G2) |
| G6 — Disable-currency | 6 ed | 1.5 wk | 1 wk (after G1+G2) |
| G7 — Off-boarding | 6 ed | 1.5 wk | 1 wk (parallel with G6) |
| G8 — Fee inference | 5 ed | 1 wk | 1 wk (after G2) |
| **Subtotal eng** | **46 ed** | **~10 wk solo** | **~5 wk parallel** |
| External audit | n/a | 3 wk | 3 wk |
| Testnet soak | n/a | 2 wk | 2 wk |
| **Total to mainnet** | | **~15 wk** | **~10 wk** |

This matches the §10 6-week target only with parallelized engineering AND aggressive audit turnaround. Realistic mainnet date is **week 10-12 from start of G0**, slightly past the 6-week design-doc target. Worth flagging.

---

## G-handoff stub-removal checklist

G0 deliberately ships scaffolding with stubs that subsequent groups replace. This table enumerates every G0 stub by file:line and which group's PR removes it. Each downstream PR's review checklist should include verification that the listed stubs are gone (replaced with real logic) and that the corresponding `**G0 stub:**` / `**G0 status:**` markers in code/comments are removed.

| Stub | File:line (G0) | Removed by | Replacement |
|---|---|---|---|
| `MIP20IssuerRegistry::is_approved_issuer` returns `false` for everyone | [mip20_issuer_registry/mod.rs:46](../crates/evm/precompiles/src/mip20_issuer_registry/mod.rs#L46) | **G4** | Real lookup from `approved_issuers[currency][issuer]` storage |
| `MIP20IssuerRegistry::get_approved_issuers` returns empty vec | [mip20_issuer_registry/mod.rs:53](../crates/evm/precompiles/src/mip20_issuer_registry/mod.rs#L53) | **G4** | Iterate `approved_issuer_list[currency]` storage |
| Governance `addApprovedIssuer` / `removeApprovedIssuer` return `Fatal` error | [mip20_issuer_registry/dispatch.rs:35-42](../crates/evm/precompiles/src/mip20_issuer_registry/dispatch.rs#L35-L42) | **G4** | EIP-712 signature verification + state mutation; emit `IssuerApproved` / `IssuerRevoked` events |
| `MIP20IssuerRegistry` struct has no storage fields | [mip20_issuer_registry/mod.rs:32-33](../crates/evm/precompiles/src/mip20_issuer_registry/mod.rs#L32-L33) | **G4** | Add `approved_issuers: Mapping<String, Mapping<Address, bool>>` + `approved_issuer_list: Mapping<String, Vec<Address>>` + `_nonce_used: Mapping<bytes32, bool>` |
| ~~`currency_registry.rs` has only validator + struct definition; no storage~~ | ~~currency_registry.rs~~ | ✅ **Removed in G1 (2026-04-28)** | `Mapping<B256, CurrencyConfig>` storage on FeeManager + `addCurrency` / `enableCurrency` / `setGovernanceAdmin` shipped. Genesis seeds USD on testnet, USD+VND on mainnet. |
| `CurrencyConfig` struct has 4 fields (G1 added `registered`); needs 7 for G6 | [mip_fee_manager/currency_registry.rs](../crates/evm/precompiles/src/mip_fee_manager/currency_registry.rs) | **G6** | Extend with `deprecating: bool`, `deprecation_activates_at: u64`, `last_pruned_at_block: u64` for the disable-currency hybrid |
| `disableCurrency` / `emergencyDisableCurrency` / `pruneCurrency` not declared | (not yet present) | **G6** | Add to `IFeeManager` ABI + implement in `MipFeeManager` |
| ~~`FeeManagerError::CurrencyNotRegistered` constructor exists, never called~~ | ~~mip_fee_manager.rs~~ | ✅ **Removed in G1 (2026-04-28)** | Now emitted by `enableCurrency` (currency unregistered) and `validate_supported_currency` (unregistered token currency). |
| `FeeManagerError::CurrencyDisabled` constructor exists, never called | [mip_fee_manager.rs](../crates/evm/contracts/src/precompiles/mip_fee_manager.rs) | **G6** | Called from grace-expiry check in `settle_fee` and from emergency-disable path |
| `FeeManagerError::FeeTokenNotAccepted` constructor exists, never called | [mip_fee_manager.rs](../crates/evm/contracts/src/precompiles/mip_fee_manager.rs) | **G2/G3** | Called from `swap_fee` revert when validator's accept-set lacks the user's token |
| `FeeManagerError::FeeTokenNotInferable` constructor exists, never called | [mip_fee_manager.rs](../crates/evm/contracts/src/precompiles/mip_fee_manager.rs) | **G8** | Called from `infer_fee_token` when calldata cannot be parsed |
| `FeeManagerError::ValidatorAcceptSetEmpty` constructor exists, never called | [mip_fee_manager.rs](../crates/evm/contracts/src/precompiles/mip_fee_manager.rs) | **G2** | Called from `get_validator_token` when validator has no tokens (replaces removed `DEFAULT_FEE_TOKEN` fallback) |
| `MIP20IssuerRegistryError` registered in error decoder but no path emits | [error.rs](../crates/evm/precompiles/src/error.rs) | **G4** | Emitted from registry governance functions |
| T4 wiring resolves `MIP20_ISSUER_REGISTRY_ADDRESS` to a stub precompile | [lib.rs:140-145](../crates/evm/precompiles/src/lib.rs#L140-L145) | **G4** | Stub precompile is replaced when G4 lands real logic; the wiring itself stays |
| Factory does NOT yet call `IssuerRegistry.is_approved_issuer` | [mip20_factory/mod.rs:104-161](../crates/evm/precompiles/src/mip20_factory/mod.rs#L104-L161) | **G4** | Add the gate at the top of `create_token`; bubble up `MIP20IssuerRegistryError::issuer_not_approved` |

**Per-group exit criteria addition:** before merging any of G1/G2/G3/G4/G6/G8, run:

```bash
# Verify the rows for this group's "Removed by" column no longer match the source
grep -rn "G0 stub\|G0 status" crates/evm/precompiles/src/<group's affected files>
```

If any G0-stub markers remain in the affected files after the group's PR, the PR is incomplete. Reviewer should flag.

**Stubs explicitly NOT removed by any group:**

The G0 inline doc-comments referencing the design doc (e.g. "see `multi-currency-fees-design.md` §4") stay forever — those are durable references, not stubs. Only `**G0 stub:**` / `**G0 status:**` markers are stub indicators.

---

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Storage migration breaks existing testnet state | Med | Med | Plan for testnet re-genesis when G2 lands; mainnet is greenfield so unaffected |
| Audit findings require G6/G7 redesign | Low | High | Bake in 2-week buffer; audit-driven changes can be stacked |
| `_try_safe_transfer` bug allows lost funds | Low | Catastrophic | Reference well-audited patterns (Compound, Aave); fuzz extensively; auditor focus |
| Magnus Foundation regulatory readiness for MagnusUSD | Med | High | Decouple legal track from engineering; engineering ships, legal closes regulatory path in parallel |
| VN partner bank readiness for vndBank | Med | High | Same; if not ready, launch USD-only and add VND post-launch |
| Multisig signer coordination on launch day | Low | Med | Pre-coordinate; signers physically together for the 15-min launch window |
| Vendor IssuerRegistry signature scheme conflict | Low | Med | Reuse existing 5-of-9 EIP-712 pattern from MBS spec; no novel cryptography |

---

## Out of scope for this plan

- **MBS bridge integration.** Separate workstream on its own feature branch (`feat/mbs-phase-0`); see `docs/superpowers/plans/2026-04-24-mbs-phase-0-prereqs.md` on that branch. Not present in `feat/multi-currency-fees` by design — the two tracks are independent.
- **`@magnus/wallet-sdk` development.** Post-launch deliverable; not blocking T4.
- **Mobile SDKs** (Swift, Kotlin). 90 days post-launch.
- **External wallet integration partnerships** (Trust Wallet, Coin98, MetaMask). BD work, not engineering.
- **Issuer-allowlist UI tooling** (web admin dashboard for multisig). v1 is multisig-call-only; tooling deferred.
- **Future: DEX-based cross-currency fee routing.** §13 of design doc.
- **Future: Per-user fee-token preferences.** §13.
- **Future: Per-token rate limits per validator.** §13.

---

## Implementation hand-off checklist

Before engineering picks up this plan, verify:

- [ ] Design doc v3.8.2 reviewed and signed off by Magnus Foundation governance
- [ ] Audit firm engaged, scope agreed
- [ ] Magnus Foundation legal/regulatory track for MagnusUSD started in parallel
- [ ] VN partner bank BD discussion confirmed; expected close date set
- [ ] Multisig signer roster confirmed for IssuerRegistry governance
- [ ] Chain ID + naming decision finalized (or accepted as carry-forward from existing 4217/42431 pending separate decision)
- [ ] T4 activation timestamp tentatively set in chainspec constants
- [ ] CI capacity available for Foundry-style fuzzing of new precompiles

When all checkboxes are green, engineering kicks off G0.

---

## Change log

- **2026-04-28 (v1.4):** G3 revised — explicit G3a/G3b split required because of pre-T4 sync requirements. G2b similarly flagged as hardfork-gated work (cannot raw-delete `user_tokens` since `revm/handler.rs` reads it on the pre-T4 path).
- **2026-04-28 (v1.3):** G2a landed (additive half of G2; per the v1 split-recommendation). `MipFeeManager` now has `validator_accepted_tokens: Mapping<Address, Mapping<Address, bool>>` + `validator_token_list: Mapping<Address, Vec<Address>>`. New API: `addAcceptedToken` / `removeAcceptedToken` / `acceptsToken` / `getAcceptedTokens` / `isAcceptedByAnyValidator`. `MAX_ACCEPT_SET_SIZE = 32` cap. Legacy `set_validator_token` / `get_validator_token` / `user_tokens` retained for backward compat — G2b removes them and rewires the fee path. `add_accepted_token` validates token is a registered+enabled MIP-20 (calls G1's `validate_supported_currency`) and respects the same-block-as-beneficiary protection. `is_accepted_by_any_validator` is a stub returning `false` until G2b/G4 add a reverse-index mapping. 13 new unit tests.
- **2026-04-28 (v1.2):** G1 landed. `MipFeeManager` now has `governance_admin` + `supported_currencies` storage; `addCurrency`/`enableCurrency`/`setGovernanceAdmin` governance functions; `getCurrencyConfig`/`isCurrencyEnabled`/`governanceAdmin` views; `validate_supported_currency` helper in `mip20/mod.rs`. `CurrencyConfig` gained an explicit `registered` flag because `added_at_block == 0` is indistinguishable from default state. Genesis-init populates USD (testnet) or USD+VND (mainnet) per chain ID. 22 new unit tests. G-handoff checklist updated to mark G1 stubs as resolved.
- **2026-04-28 (v1.1):** Added "G-handoff stub-removal checklist" section enumerating every G0 stub by file:line and the group whose PR removes it. Per-group exit criteria includes a `grep` check for residual G0-stub markers. Clarified that durable design-doc cross-references stay; only `**G0 stub:**` / `**G0 status:**` markers are removed.
- **2026-04-27 (v1):** Initial implementation plan covering 8 deliverable groups with effort estimates, dependency ordering, audit checkpoint placement, testnet/mainnet sequencing, risk register.
