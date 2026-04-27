# Multi-Currency Fees, Issuer-Allowlist & Foundation-Issuer Model — Design Specification

**Status:** Draft v3.8.2
**Date:** 2026-04-27
**Scope:** Define Magnus's complete fee, issuance, and validator model for v1 mainnet.

**Key v3.8.2 decisions baked in (cumulative across v3.5–v3.8.2):**

1. **Multi-currency fees.** Any registered fiat currency (USD, VND, EUR, …) can be used for gas. Per-currency governance enable/disable.
2. **Issuer allowlist (per-currency).** Only Magnus-governance-approved addresses can deploy MIP-20s of a given currency. Permissionless deployment is **disabled** in v1.
3. **Magnus Foundation as bootstrap issuer.** Foundation deploys `MagnusUSD` (symbol `mUSD`) at genesis at reserved address `0x20C0…0010`, holding real USD reserves. Real stablecoin, not a stub. Slot `0x20C0…0000` (the inherited Tempo `MagnusUSD` slot) is left empty in genesis.
4. **Multi-token validator accept-set.** Validator-orgs accept a set of tokens, not a single one. Receive payouts as a basket.
5. **Wallet-inferred fee token.** No per-user fee-token state. Wallet picks fee token per-tx based on what's being moved.
6. **No DEX, no Fee AMM in fee path.** Fee collection is direct-credit-or-revert. Fee AMM precompile deleted.
7. **Mainnet launch with USD + VND from day 1, ~6-week delay** to coordinate Magnus Foundation issuance, VN partner bank issuance, and Foundation validator-org setup.

---

## 1. Problem and divergence from Tempo

### 1.1 What Magnus inherited from Tempo

Tempo hard-codes USD as the only fee-payable currency, restricts validators and users to a single token each, routes cross-token same-currency conversion through a Fee AMM, and uses `MagnusUSD` (issued by Bridge, Stripe-owned) as the canonical USD anchor.

| Behavior | File | Magnus replaces with |
|---|---|---|
| `validate_usd_currency` gate | [`mip20/mod.rs:53-58`](../crates/evm/precompiles/src/mip20/mod.rs#L53-L58) | `validate_supported_currency` (registry-driven) |
| `setValidatorToken(single)` | [`mip_fee_manager/mod.rs:80-108`](../crates/evm/precompiles/src/mip_fee_manager/mod.rs#L80-L108) | `addAcceptedToken / removeAcceptedToken` (multi-token) |
| `setUserToken(single)` per-user state | [`mip_fee_manager/mod.rs:116-145`](../crates/evm/precompiles/src/mip_fee_manager/mod.rs#L116-L145) | Removed entirely; wallet infers |
| `MAGNUS_USD_ADDRESS = MagnusUSD` fallback | [`mip_fee_manager/mod.rs:60-68`](../crates/evm/precompiles/src/mip_fee_manager/mod.rs#L60-L68) | Removed; fail-closed |
| Fee AMM swap inside `swap_fee` | [`mip_fee_manager/mod.rs:209-222`](../crates/evm/precompiles/src/mip_fee_manager/mod.rs#L209-L222) | Removed; direct credit only |
| Cross-currency quoteToken rejection | [`mip20_factory/mod.rs:126-130`](../crates/evm/precompiles/src/mip20_factory/mod.rs#L126-L130) | Same-currency rule retained, generalized; `quoteToken = 0` allowed |
| Permissionless `createToken` | [`mip20_factory/mod.rs:104-161`](../crates/evm/precompiles/src/mip20_factory/mod.rs#L104-L161) | **Issuer allowlist gate added** via new `MIP20IssuerRegistry` precompile |
| `MagnusUSD` at `0x20C0…0000` (Bridge-issued) | Genesis JSON | `MagnusUSD` (Magnus Foundation-issued) at `0x20C0…0010`; slot `0x20C0…0000` left empty |

### 1.2 Magnus's chosen model

**Permissioned issuance, multi-currency fees, multi-token validator acceptance, wallet-inferred fee selection, foundation as bootstrap stablecoin issuer.** Issuance is gated; fees are flexible.

## 2. Architecture

### 2.1 Entities

| Entity | Role |
|---|---|
| **Magnus Foundation** | Issues `MagnusUSD` at genesis (real reserves, Option B from prior decision). Runs all validators initially. Operates the governance multisig that controls IssuerRegistry + currency registry. |
| **Approved issuer** | A non-Foundation entity (Tether, Circle, partner banks) who has been added to `IssuerRegistry.approvedIssuers[currency]` by governance. Can deploy MIP-20s of approved currencies via the factory. |
| **Issuer's token** | An MIP-20 (USDT, USDC, vndBank, …) deployed by an approved issuer. Functionally equivalent to MagnusUSD; no special protocol status. |
| **User** | Holds tokens; wallet selects fee token per-tx automatically. No per-user state in protocol. |
| **Validator-org** | Magnus Foundation initially. Future: partner orgs. Each runs N nodes, each node has an accept-set of tokens it will receive payouts in. |

### 2.2 The on-chain stack

```
┌─────────────────────────────────────────────────────────┐
│  IMIP20IssuerRegistry         @ 0x20FA000…       (NEW)  │
│  Per-currency allowlist of approved issuer addresses    │
└─────────────────────────────────────────────────────────┘
                 │ checked by
                 ▼
┌─────────────────────────────────────────────────────────┐
│  IMIP20Factory                @ 0x20FC000…              │
│  createToken (now gated by IssuerRegistry)              │
└─────────────────────────────────────────────────────────┘
                 │ deploys
                 ▼
┌─────────────────────────────────────────────────────────┐
│  MIP-20 tokens                @ 0x20C0…<deterministic>  │
│  MagnusUSD @ 0x20C0…0010 (genesis)                      │
│  USDT, USDC, vndBank, ... @ derived addresses           │
└─────────────────────────────────────────────────────────┘
                 │ accepted by
                 ▼
┌─────────────────────────────────────────────────────────┐
│  IFeeManager                  @ 0xfeec000…              │
│  Currency registry (USD, VND, …)                        │
│  Validator accept-sets (multi-token per validator)      │
│  Fee collection (direct credit, no AMM, no DEX)         │
└─────────────────────────────────────────────────────────┘
                 │ uses (off fee path)
                 ▼
┌─────────────────────────────────────────────────────────┐
│  IStablecoinDEX               @ 0xdec0000…              │
│  Orderbook for general trading. NOT in fee path.        │
└─────────────────────────────────────────────────────────┘
```

**(NEW) = new precompile introduced in this spec (v3.5+).**

### 2.3 Validators are partner organizations

Magnus Foundation runs all validators at launch. Future partner orgs (banks, fintechs) join via BD agreements. Each validator-org's accept-set is a commercial decision, not a per-tx user concern.

Sample validator landscape after partner onboarding:

| Validator-org | Accept-set | Notes |
|---|---|---|
| Magnus Foundation | `{MagnusUSD, USDT, USDC, vndBank, eurBank}` | Universal coverage |
| Tether (future) | `{USDT, USDC, MagnusUSD}` | Own product + diversification |
| VN Partner Bank (future) | `{vndBank, MagnusUSD}` | VND-primary + USD ops |

## 3. Currency registry (FeeManager)

### 3.1 Storage

```solidity
// In MipFeeManager:
struct CurrencyConfig {
    bool   enabled;         // gas-eligible?
    uint64 addedAtBlock;
    uint64 enabledAtBlock;
}
mapping(string => CurrencyConfig) supportedCurrencies;  // ISO 4217 code
```

### 3.2 Governance functions

```solidity
function addCurrency(string code, /* gov sig fields */) external;
function enableCurrency(string code, /* gov sig fields */) external;
function disableCurrency(string code, /* gov sig fields */) external;
```

All use the existing 5-of-9 multisig EIP-712 pattern.

### 3.3 Genesis state

| Network | Initial registry |
|---|---|
| Testnet (Moderato or replacement) | `{USD: enabled}` |
| Mainnet (per #4 launch decision) | `{USD: enabled, VND: enabled}` |

Future currencies require governance `addCurrency` + `enableCurrency`.

## 4. Issuer registry — new precompile

### 4.1 New precompile: `IMIP20IssuerRegistry`

**Address:** `0x20FA000000000000000000000000000000000000`

(Adjacent to `MIP20_FACTORY_ADDRESS = 0x20FC…`. Reads as the issuer-side counterpart to the factory.)

### 4.2 Storage

```solidity
mapping(string currency => mapping(address issuer => bool)) approvedIssuers;
mapping(string currency => address[]) approvedIssuerList;  // for enumeration
```

### 4.3 ABI

```solidity
interface IMIP20IssuerRegistry {
    function isApprovedIssuer(string memory currency, address issuer)
        external view returns (bool);

    function getApprovedIssuers(string memory currency)
        external view returns (address[] memory);

    function addApprovedIssuer(
        string memory currency,
        address issuer,
        uint64 nonce,
        uint64 expiresAt,
        bytes calldata governanceSig
    ) external;

    function removeApprovedIssuer(
        string memory currency,
        address issuer,
        uint64 nonce,
        uint64 expiresAt,
        bytes calldata governanceSig
    ) external;

    event IssuerApproved(string currency, address indexed issuer);
    event IssuerRevoked(string currency, address indexed issuer);

    error IssuerNotApproved(address issuer, string currency);
    error IssuerAlreadyApproved(address issuer, string currency);
    error CurrencyNotRegistered(string currency);
    error InvalidGovernanceSignature();
}
```

### 4.4 Genesis state

**Empty.** No issuers pre-approved. Magnus Foundation must call `addApprovedIssuer` via multisig after mainnet launch to authorize itself, then anyone else.

### 4.5 EIP-712 signing pattern

Reuses Magnus's established governance multisig pattern (5-of-9, per #6). Per-action typehashes:

```solidity
bytes32 ADD_APPROVED_ISSUER_TYPEHASH = keccak256(
    "AddApprovedIssuer(string currency,address issuer,uint64 nonce,uint64 expiresAt)"
);
bytes32 REMOVE_APPROVED_ISSUER_TYPEHASH = keccak256(
    "RemoveApprovedIssuer(string currency,address issuer,uint64 nonce,uint64 expiresAt)"
);
```

Domain separator: standard EIP-712 with `(name="MIP20IssuerRegistry", version="1", chainId, address)`.

Per-digest nonce replay protection in `_nonceUsed` mapping. Expiry timestamp prevents indefinite replay.

### 4.6 Per-currency scope

Approval is scoped to a single currency. Tether being approved for USD does not authorize them to deploy VND tokens. Each (issuer, currency) pair requires its own approval.

This means:
- A multi-currency issuer (e.g. Circle wanting both USDC and EURC) requires two governance approvals.
- Revoking an issuer for one currency doesn't affect them in another.
- Auditing is clearer: "who's allowed to deploy USD?" returns a finite list per currency.

## 5. MagnusUSD — Foundation-issued bootstrap token

### 5.1 Identity

- **Name:** `MagnusUSD`
- **Symbol:** `mUSD`
- **Decimals:** 6 (per MIP-20 standard)
- **Address:** `0x20C0000000000000000000000000000000000010` (reserved slot 0x010). Distinct from the inherited Tempo `MagnusUSD` slot `0x20C0…0000`, which is left empty in Magnus genesis. Magnus Foundation reserves the `0x010`–`0x0FF` band for its own stablecoin family (future `MagnusVND`, `MagnusEUR`, etc. if ever issued); `0x000`–`0x00F` stays free for protocol-utility tokens.
- **Issuer:** Magnus Foundation
- **Currency:** `"USD"`
- **Quote token:** `address(0)` (first-of-currency at genesis)
- **Reserves:** real USD held at a banking partner, 1:1 backing
- **Mint/redeem:** Magnus Foundation operations team via banking rails (off-chain), reflected on-chain via `ISSUER_ROLE` on the token

### 5.2 Why Magnus Foundation issues it directly (Option B)

- **Bootstrap anchor:** the first USD MIP-20 on Magnus mainnet, exists from block 1, allows other issuers to use as quote token (`Tether deploys USDT with quoteToken = MagnusUSD`).
- **Universal fee fallback:** Magnus Foundation validator-orgs accept `MagnusUSD` in their accept-set; users with mUSD can always pay fees.
- **No partner dependency:** Magnus doesn't need a Bridge-equivalent partner before launch.
- **Regulatory load:** Magnus Foundation must obtain MTL/MSB licensing OR operate from a non-US jurisdiction with stablecoin authorization. This is the trade-off vs. Options A/C from the prior discussion.

### 5.3 Genesis deployment mechanism

`MagnusUSD` is deployed at genesis via the existing `create_token_reserved_address` path in [`mip20_factory/mod.rs:172-235`](../crates/evm/precompiles/src/mip20_factory/mod.rs#L172-L235), which:

- Uses reserved address slots `0x20C0…0000`–`0x20C0…03FF`
- Bypasses the public factory's `quoteToken` requirement (allows `address(0)`)
- Is governance-only — only Magnus Foundation can call

**MagnusUSD bypasses the IssuerRegistry check.** This is intentional and explicit:

- `IssuerRegistry` checks apply only to the public `createToken` path.
- Reserved-address tokens are deployed at genesis or via governance-only paths, not by individual issuers.
- Magnus Foundation is implicitly the issuer of MagnusUSD by virtue of operating the genesis allocator.
- This is documented openly; not a hidden privilege.

### 5.4 Roles and admin

- `DEFAULT_ADMIN_ROLE` → Magnus Foundation multisig
- `ISSUER_ROLE` → Magnus Foundation treasury multisig (mint/burn against reserves)
- `PAUSE_ROLE` → Magnus Foundation security-ops multisig
- Optional MIP-403 transfer policy: TBD per regulatory requirements

### 5.5 Constant rename

In [`crates/evm/contracts/src/precompiles/mod.rs`](../crates/evm/contracts/src/precompiles/mod.rs):

```rust
// Old (Tempo-inherited):
pub const MAGNUS_USD_ADDRESS: Address = address!("0x20C0000000000000000000000000000000000000");

// New (Magnus v3.8.1):
pub const MAGNUS_USD_ADDRESS: Address = address!("0x20C0000000000000000000000000000000000010");
```

The constant is renamed AND the address moves from slot `0x000` to slot `0x010`. Slot `0x20C0…0000` is left empty in Magnus genesis to provide a clean break from inherited Tempo `MagnusUSD` state.

All references to `MAGNUS_USD_ADDRESS` in the codebase update to `MAGNUS_USD_ADDRESS` with the new value. Reserved-range slot-0 special-cases in factory code (e.g. [`mip20_factory/mod.rs:197`](../crates/evm/precompiles/src/mip20_factory/mod.rs#L197) which hard-codes `address == MAGNUS_USD_ADDRESS` to require zero quote token) are **generalized** to the first-of-currency rule from §4.4a, no longer dependent on a specific slot.

## 6. Validator accept-sets

### 6.1 Storage (replaces single-token preference)

```rust
// In MipFeeManager:
validator_accepted_tokens: mapping(address validator => mapping(address token => bool));
validator_token_list:      mapping(address validator => Vec<address>);  // enumeration
```

Old `validator_tokens: mapping(address => address)` is deleted.

### 6.2 ABI

```solidity
function addAcceptedToken(address token) external;
function removeAcceptedToken(address token) external;
function acceptsToken(address validator, address token) external view returns (bool);
function getAcceptedTokens(address validator) external view returns (address[] memory);
function isAcceptedByAnyValidator(address token) external view returns (bool);

event AcceptedTokenAdded(address indexed validator, address indexed token);
event AcceptedTokenRemoved(address indexed validator, address indexed token);

error FeeTokenNotAccepted(address validator, address token);
error MaxAcceptSetReached(address validator);
error TokenNotInAcceptSet(address validator, address token);
```

### 6.3 Constraints

- `addAcceptedToken(token)` requires:
  - `is_tip20(token)` — token is deployed
  - `validate_supported_currency(token)` — token's currency is registered + enabled
- Max accept-set size: `MAX_ACCEPT_SET_SIZE = 32` (per #10).
- Same-block-as-beneficiary protection (existing `cannot_change_within_block` logic) applies.

### 6.4 Validator-org always-set commitment

Off-chain BD requirement: every validator-org commits to having a non-empty accept-set before producing blocks. Operationally enforced; not protocol-checked. A validator-org with empty accept-set still produces blocks but can't collect fees.

## 7. Fee-collection flow (direct credit, no AMM, no DEX)

### 7.1 Two cases

At fee-collection time with `feeToken = X` (set by wallet, see §8) and producing validator-org `V`:

**Case A — Validator accepts the token:** direct credit. `V.collectedFees[X] += feeAmount`. No swap.

**Case B — Validator does not accept the token:** revert with `FeeTokenNotAccepted`. Tx fails. Wallet handles retry or shows error.

### 7.2 No Fee AMM, no DEX in path

- **Fee AMM precompile is deleted entirely** ([`mip_fee_manager/amm.rs`](../crates/evm/precompiles/src/mip_fee_manager/amm.rs) and the `TIPFeeAMM` interface). Per #9.
- Stablecoin DEX remains operational for general trading but is not called by FeeManager.

### 7.3 Validator's payout balance is a basket

`distributeFees(validator, token)` works per-token. Each validator-org may accumulate balances in many tokens. They consolidate off-chain or via Stablecoin DEX.

## 8. Fee-token selection — wallet-inferred

### 8.1 Inference rules (wallet-side)

| Tx pattern | Inferred fee token |
|---|---|
| Direct MIP-20 transfer (`USDT.transfer(...)`) | The token being transferred |
| Multi-token swap (`Router.swap(USDT → USDC)`) | The token being spent |
| Contract call moving no tokens | User's largest-balance registered MIP-20 |
| User explicitly overrides | User's choice |

### 8.2 Pre-submission validation

Wallet must verify before submission:
- User holds enough of the inferred fee token.
- At least one validator-org accepts the inferred token (`isAcceptedByAnyValidator(token)`).

If either fails, wallet shows error or suggests alternative.

### 8.3 No `userTokens` storage

`userTokens` mapping, `set_user_token` precompile function, and `userTokens` view are **removed entirely**. No per-user fee-token state in protocol.

## 9. Required code changes

### 9.1 NEW: [`crates/evm/precompiles/src/mip20_issuer_registry/`](../crates/evm/precompiles/src/mip20_issuer_registry/)

- New precompile module: `mod.rs`, `dispatch.rs`.
- Storage: `approvedIssuers[currency][issuer] → bool`, `approvedIssuerList[currency] → Vec<address>`.
- Governance functions per §4.3.
- Address: `0x20FA000000000000000000000000000000000000`.
- Add to [`extend_magnus_precompiles`](../crates/evm/precompiles/src/lib.rs#L114) at startup.

### 9.2 NEW: [`crates/evm/contracts/src/precompiles/mip20_issuer_registry.rs`](../crates/evm/contracts/src/precompiles/mip20_issuer_registry.rs)

- ABI definition for `IMIP20IssuerRegistry` per §4.3.

### 9.3 Modified: [`crates/evm/contracts/src/precompiles/mod.rs`](../crates/evm/contracts/src/precompiles/mod.rs)

- Add `pub const MIP20_ISSUER_REGISTRY_ADDRESS: Address = address!("0x20FA000000000000000000000000000000000000");`
- Rename `MAGNUS_USD_ADDRESS` → `MAGNUS_USD_ADDRESS` AND change value from `0x20C0…0000` to `0x20C0…0010`.
- Update `MAGNUS_USD_ADDRESS` constant — recommendation: delete entirely (no callers after FeeManager changes).

### 9.4 Modified: [`crates/evm/precompiles/src/mip20_factory/mod.rs`](../crates/evm/precompiles/src/mip20_factory/mod.rs)

- **Add IssuerRegistry check** in `create_token` (public path, NOT reserved-address path):
  ```rust
  let registry = MIP20IssuerRegistry::new();
  if !registry.is_approved_issuer(&call.currency, sender)? {
      return Err(MIP20IssuerRegistryError::issuer_not_approved(sender, call.currency.clone()).into());
  }
  ```
- Relax cross-currency quote rule (lines 126-130, 201-205): "currency must match quoteToken's currency, IF quoteToken is non-zero."
- Allow `quoteToken == address(0)` in public factory (first-of-currency case still useful for non-Foundation issuers).
- The factory bubbles up the IssuerRegistry's `IssuerNotApproved(address, string)` error directly. **Do NOT** add a duplicate `IssuerNotApproved` to `IMIP20Factory` — Solidity assigns the same 4-byte selector to identically-named errors with the same signature, which would ambiguate the global decoder registry.

### 9.5 Modified: [`crates/evm/precompiles/src/mip20/mod.rs`](../crates/evm/precompiles/src/mip20/mod.rs)

- Add `validate_supported_currency(token: Address) -> Result<()>`.
- Keep `validate_usd_currency` only if needed (audit; likely none).

### 9.6 Modified: [`crates/evm/precompiles/src/mip_fee_manager/mod.rs`](../crates/evm/precompiles/src/mip_fee_manager/mod.rs)

- Add `supportedCurrencies` storage map (§3.1).
- Add governance functions `addCurrency`, `enableCurrency`, `disableCurrency` (§3.2).
- Replace `validator_tokens` with `validator_accepted_tokens` + `validator_token_list` (§6.1).
- Replace `set_validator_token` with `add_accepted_token` + `remove_accepted_token` (§6.2).
- Replace `get_validator_token` with `accepts_token` + `get_accepted_tokens`.
- Remove `MAGNUS_USD_ADDRESS` fallback. Fail-closed.
- Remove `set_user_token`, `userTokens`, view functions for user tokens.
- Replace `swap_fee` AMM-call path with direct-credit-or-revert.
- Delete `mip_fee_manager/amm.rs` and `TIPFeeAMM` interface entirely.
- Add errors per §6.2.
- Add `MAX_ACCEPT_SET_SIZE = 32` constant.
- Add views per §6.2.

### 9.7 Modified: [`crates/evm/contracts/src/precompiles/mip_fee_manager.rs`](../crates/evm/contracts/src/precompiles/mip_fee_manager.rs)

- Update `IFeeManager` ABI per all the changes above.
- Drop `setUserToken`, `userTokens`, `setValidatorToken`, `validatorTokens` events and errors.
- Drop `ITIPFeeAMM` entirely.

### 9.8 Modified: [`crates/primitives/chainspec/src/genesis/*.json`](../crates/primitives/chainspec/src/genesis/)

- **Remove** the existing `MagnusUSD` allocation at `0x20c0000000000000000000000000000000000000`. Slot is left empty (no bytecode, no storage).
- **Add** new `MagnusUSD` allocation at `0x20c0000000000000000000000000000000000010`:
  - `name = "MagnusUSD"`, `symbol = "mUSD"`, `currency = "USD"`, `quoteToken = 0x0`, `admin = magnusFoundationMultisig`, supply cap configurable.
- **Update** genesis `supportedCurrencies` config in `MipFeeManager` storage:
  - Testnet: `{USD: enabled}`
  - Mainnet: `{USD: enabled, VND: enabled}`
- **Reserved range** `0x20C0…0000`–`0x20C0…03FF` stays reserved. Within that range:
  - `0x000`–`0x00F`: free, reserved for future protocol-utility tokens
  - `0x010`: `MagnusUSD` (this spec)
  - `0x011`–`0x0FF`: reserved for the Magnus Foundation stablecoin family (future `MagnusVND`, `MagnusEUR`, etc.) if ever issued
  - `0x100`–`0x3FF`: reserved for other future protocol use

### 9.9 No change: [`crates/primitives/chainspec/src/constants.rs`](../crates/primitives/chainspec/src/constants.rs)

- Base fee `MAGNUS_T1_BASE_FEE = 20_000_000_000` attodollars stays.

## 10. Launch sequence (mainnet, per #4 = Path A)

### 10.1 Pre-launch (weeks 0-5)

| Week | Magnus team | Validator side | Issuer side |
|---|---|---|---|
| 0-1 | Code: IssuerRegistry, FeeManager refactor, factory gate. Tests. | — | — |
| 1-2 | Code review, security review. | — | BD: Tether/Circle outreach. VN bank outreach. |
| 2-3 | Deploy to testnet. Validator-orgs run testnet nodes. | Foundation runs Moderato testnet validators. | First testnet issuance: Magnus Foundation deploys testnet `mUSD`. |
| 3-4 | Audit external. Fix audit findings. | Foundation tests accept-set add/remove flows. | Testnet partner issuances: testnet vndBank, possibly testnet USDT. |
| 4-5 | Final testing. Mainnet build pinned. Operations docs finalized. | Foundation provisions mainnet validator nodes. | Foundation prepares mUSD reserves. VN bank prepares vndBank reserves. |

### 10.2 Launch day (week 6)

```
T-0:00  Mainnet block 1 produced. State:
        - supportedCurrencies = {USD: enabled, VND: enabled}
        - approvedIssuers = {} (empty)
        - validator accept-sets = {} (empty)
        - MagnusUSD already deployed at 0x20C0…0010 (genesis allocation)
        - 0x20C0…0000 (inherited Tempo MagnusUSD slot) is empty

T+0:01  Foundation multisig calls (pre-coordinated, signers ready):
        - IssuerRegistry.addApprovedIssuer("USD", magnusFoundationAddr)
        - IssuerRegistry.addApprovedIssuer("USD", tetherAddr)        [if Tether ready]
        - IssuerRegistry.addApprovedIssuer("USD", circleAddr)        [if Circle ready]
        - IssuerRegistry.addApprovedIssuer("VND", vnPartnerBankAddr)

T+0:05  Approved issuers deploy:
        - Magnus Foundation: MagnusUSD (already at genesis; mints initial supply against reserves)
        - VN partner bank: vndBank token via factory (quoteToken = 0x0)
        - Tether (if ready): USDT via factory (quoteToken = MagnusUSD)
        - Circle (if ready): USDC via factory (quoteToken = MagnusUSD or USDT)

T+0:10  Foundation validator-orgs configure accept-sets:
        - addAcceptedToken(MagnusUSD)
        - addAcceptedToken(vndBank)
        - addAcceptedToken(USDT)  [if deployed]
        - addAcceptedToken(USDC)  [if deployed]

T+0:15  Fees flow. First user transactions succeed.
```

### 10.3 Launch-day risks

- **Multisig signer coordination.** Pre-coordinate: signers physically available, hardware wallets ready, payloads pre-signed where possible. Any signer absent = launch delay.
- **Reserve banking timing.** Magnus Foundation must have USD reserves at banking partner before mainnet (legally required to back mUSD).
- **Audit findings.** External audit may surface issues requiring code changes. Buffer week 4-5 absorbs minor findings; major findings push the timeline.
- **VN bank readiness.** If VN partner bank is not ready at week 6, two paths: delay launch further OR launch USD-only and add VND post-launch (reverses #4 decision). Prefer delay.

### 10.4 Post-launch milestones

- Week 7-12: monitor, fix issues, onboard additional issuers (Tether/Circle if not at launch).
- Month 3-6: onboard first non-Foundation validator-org (a partner bank running validator nodes).
- Month 6-12: progressively decentralize validator set; Foundation share drops below 50%.

## 11. Open questions still unresolved

These are explicitly NOT decided:

### 11.1 Disable-currency semantics — RESOLVED (v3.6)

Hybrid mechanism: standard deprecation with grace period (Option C) for normal use, emergency hard-stop (Option A) for security/regulatory crises, optional `pruneCurrency` (Option B) for state cleanup. Both grace duration and emergency threshold are governance-tunable.

**Storage additions to FeeManager:**

```solidity
struct CurrencyConfig {
    bool   enabled;                       // gas-eligible?
    bool   deprecating;                   // grace period active?
    uint64 deprecationActivatesAt;        // when grace ends (currency becomes disabled)
    uint64 addedAtBlock;
    uint64 enabledAtBlock;
    uint64 lastPrunedAtBlock;             // for paginated pruning
}

uint64 deprecationGracePeriod;            // genesis default: 30 days
uint8  emergencyDisableThreshold;         // genesis default: 7 (of 9-signer multisig)
```

**New functions:**

```solidity
function disableCurrency(string code, /* gov sig: standard threshold */) external;
function emergencyDisableCurrency(string code, /* gov sig: emergency threshold */) external;
function pruneCurrency(string code, uint256 maxIterations) external;          // anyone, paginated
function setDeprecationGracePeriod(uint64 newDuration, /* standard sig */) external;
function setEmergencyDisableThreshold(uint8 newThreshold, /* CURRENT emergency sig */) external;
```

**Critical governance-design rule:** changing the emergency threshold requires the *current* emergency threshold (not standard). Prevents 5-of-9 from secretly downgrading emergency power to themselves.

**Behavior matrix:**

| Action | Trigger | Threshold | Effect | Reversible? |
|---|---|---|---|---|
| `disableCurrency` | Standard gov | 5-of-9 (default) | Grace period starts; new factory deploys + accept-token calls blocked for that currency; existing fees still work | Yes — `enableCurrency` during grace |
| Grace expiry | Block timestamp | None | `enabled = false`; fee txs revert | Yes — `enableCurrency` re-enables |
| `emergencyDisableCurrency` | Emergency gov | 7-of-9 (default) | `enabled = false` immediately | Yes — `enableCurrency` (standard) |
| `pruneCurrency` | Anyone | None | Removes that currency's tokens from validator accept-sets, paginated | No — accept-sets must be rebuilt by validator-orgs |
| `setDeprecationGracePeriod` | Standard gov | 5-of-9 | Updates default grace for future disables | Yes |
| `setEmergencyDisableThreshold` | Emergency gov | Current threshold | Updates emergency threshold | Yes |

**Genesis defaults:**
- `deprecationGracePeriod = 30 days`
- `emergencyDisableThreshold = 7`

**Sanity bounds at setter time:**

```solidity
require(newDuration >= 1 hours && newDuration <= 365 days, "grace out of range");
require(newThreshold > 5 && newThreshold <= 9, "threshold out of range");  // strictly > standard, ≤ multisig size
```

**Events:**

```solidity
event CurrencyDisabling(string code, uint64 graceEndsAt, address by);
event CurrencyDisabledEmergency(string code, address by);
event CurrencyDisabled(string code);                              // grace expired
event CurrencyEnabled(string code, address by);
event CurrencyDeprecationWarning(string code, uint64 timeRemaining);  // emit on every fee tx during grace
event CurrencyPruned(string code, uint256 itemsRemoved);
event DeprecationGracePeriodChanged(uint64 oldDuration, uint64 newDuration);
event EmergencyDisableThresholdChanged(uint8 oldThreshold, uint8 newThreshold);
```

**Behavior during grace period (`deprecating = true, enabled = true`):**

- Fee transactions in tokens of this currency: succeed normally (with `CurrencyDeprecationWarning` event emitted).
- Factory `createToken` for this currency: reverts with `CurrencyDeprecating`.
- Validator `addAcceptedToken` for this currency: reverts with `CurrencyDeprecating`.
- Validator `removeAcceptedToken` for this currency: succeeds (validators can prepare for the deprecation).
- Issuer `mint` on existing tokens: succeeds (no protocol restriction; issuer's own discretion).

### 11.2 Validator-org churn — RESOLVED (v3.6)

Hybrid: distribute on removal, escrow excess. When a validator-org is offboarded (voluntary exit, governance removal, or consensus-layer removal), the protocol attempts direct delivery of `collectedFees` to the validator's payout address. Failed deliveries (broken recipients, blocklists, reverting tokens) route to a Magnus Foundation-controlled escrow with a claim window.

**Storage additions to FeeManager:**

```solidity
address foundationEscrowAddress;
mapping(address validator => mapping(address token => uint128)) escrowedFees;

struct ClaimRecord {
    uint64  offboardedAt;
    uint64  claimDeadline;
    bool    claimed;
}
mapping(address validator => ClaimRecord) escrowClaims;

uint64 escrowClaimWindow;            // genesis default: 365 days
```

**New functions:**

```solidity
function offboardValidator(address validator, /* standard gov sig */) external;
function claimEscrowedFees(address validator, address token, address recipient,
    uint64 nonce, uint64 expiresAt, bytes calldata validatorSig) external;
function sweepExpiredEscrow(address validator, address token, /* standard gov sig */) external;
function setEscrowClaimWindow(uint64 newWindow, /* standard gov sig */) external;
function setFoundationEscrowAddress(address newAddress, /* standard gov sig */) external;
```

**`offboardValidator` semantics:**

```
For each token in validator's accept-set:
    amount = collectedFees[validator][token]
    if amount > 0:
        delivered = _trySafeTransfer(token, validator, amount)   // bounded-gas low-level call
        if delivered:
            collectedFees[validator][token] = 0
            emit FeesOffboardDelivered
        else:
            collectedFees[validator][token] = 0
            escrowedFees[validator][token] += amount
            emit FeesOffboardEscrowed

Clear validator's accept-set entirely.
Record ClaimRecord{offboardedAt, claimDeadline = now + escrowClaimWindow}.
emit ValidatorOffboarded.
```

**`_trySafeTransfer` implementation** (bounded-gas low-level call, USDT-compatible void-return handling):

```solidity
function _trySafeTransfer(address token, address to, uint256 amount) internal returns (bool) {
    (bool ok, bytes memory data) = token.call{gas: 100_000}(
        abi.encodeWithSelector(IERC20.transfer.selector, to, amount)
    );
    if (!ok) return false;
    if (data.length == 0) return true;              // void return = assume success (USDT pattern)
    return abi.decode(data, (bool));
}
```

100k gas cap prevents griefing via OOG on malicious recipient hooks.

**`claimEscrowedFees`:** validator-org proves control of their original address via EIP-712 signature, can specify any recipient (often their original address or a successor multisig). Requires `block.timestamp <= claimDeadline`. Per-(validator, token) granularity.

**`sweepExpiredEscrow`:** governance can sweep unclaimed escrow to `foundationEscrowAddress` after claim window expires. Per-(validator, token) granularity. Funds become Foundation property; no auto-burn.

**Sanity bounds:**

```solidity
require(newWindow >= 30 days && newWindow <= 1825 days, "claim window out of range");
```

**Genesis defaults:**

- `escrowClaimWindow = 365 days`
- `foundationEscrowAddress = <Magnus Foundation multisig>` (configured at genesis)

**Events:**

```solidity
event ValidatorOffboarded(address indexed validator, uint64 claimDeadline);
event FeesOffboardDelivered(address indexed validator, address indexed token, uint256 amount);
event FeesOffboardEscrowed(address indexed validator, address indexed token, uint256 amount);
event EscrowedFeesClaimed(address indexed validator, address indexed token, address indexed recipient, uint256 amount);
event EscrowSwept(address indexed validator, address indexed token, address indexed foundation, uint256 amount);
event EscrowClaimWindowChanged(uint64 oldWindow, uint64 newWindow);
event FoundationEscrowAddressChanged(address oldAddress, address newAddress);
```

**Errors:**

```solidity
error ValidatorNotOffboarded();
error ClaimWindowExpired();
error ClaimWindowActive();
error InvalidValidatorSig();
error NoEscrowedFees();
```

**Behavior matrix:**

| Scenario | Outcome |
|---|---|
| Validator with working address, balance > 0 | Direct transfer (`FeesOffboardDelivered`) |
| Validator with broken/blocked address | Routed to escrow (`FeesOffboardEscrowed`) |
| Validator with no balance | No-op for that token; offboard proceeds |
| Validator claims escrow within window | `claimEscrowedFees` succeeds |
| Validator claims after window | Reverts `ClaimWindowExpired` |
| Foundation sweeps after window | Funds → `foundationEscrowAddress` |
| Validator rejoins after offboarding | New record; old offboard preserved for audit |

**Integration with consensus-layer validator removal:** the consensus-layer mechanism for removing a validator from the active set is separate. The recommended off-boarding sequence:

1. Consensus layer removes validator from active set (no longer produces blocks).
2. Within N blocks, governance calls `FeeManager.offboardValidator(validator, ...)`.
3. Fees delivered or escrowed per the matrix above.

If step 2 is skipped, no funds are lost — `collectedFees` remain claimable via `distributeFees` (which is permissionless). The off-boarding function provides an explicit cleanup ceremony, not a critical path.

### 11.3 Wallet inference standardization — RESOLVED (v3.8)

**Goal:** any EVM wallet (MetaMask, Trust Wallet, Coin98, Rabby, Brave, etc.) works on Magnus. **Protocol updates are required and acceptable.** Wallets do not need to know about Magnus-specific behavior; the chain handles fee-token selection at the protocol layer. SDK provides richer UX for wallets that want pre-submission validation.

**Two-layer strategy:**

#### Layer 1 — Protocol-side fee-token inference (in v1 mainnet)

`MipFeeManager` infers the fee token from the tx calldata at fee-collection time. The tx envelope retains an optional `feeToken` override field; when not set (the common case for vanilla EVM wallets), inference runs.

```rust
// In MipFeeManager — invoked during collect_fee_pre_tx
fn infer_fee_token(tx: &TxEnvelope) -> Result<Address> {
    // If wallet (or sponsor) specified feeToken explicitly, respect it
    if let Some(token) = tx.fee_token {
        return Ok(token);
    }

    let to = tx.to.ok_or(FeeManagerError::FeeTokenNotInferable)?;

    // Direct MIP-20 call (transfer, transferFrom, mint, burn, etc.)
    if to.is_tip20() {
        return Ok(to);
    }

    // Recognized router/swap selectors — governance-registered
    if let Some(token) = parse_known_router_calldata(&tx.input)? {
        return Ok(token);
    }

    Err(FeeManagerError::FeeTokenNotInferable.into())
}
```

**Tx envelope retains `feeToken` as optional override** for two valid use cases:

- **Power users / sophisticated wallets** that want to pay fees in a non-default token (e.g. user sends USDT but wants to pay fees in vndBank because they hold more vndBank).
- **Sponsored transactions / paymasters** where a DApp pays fees on behalf of the user, directing the fee to their preferred token.

Default wallet behavior: leave `feeToken` empty; let protocol infer. Vanilla EVM wallets do this naturally.

**Storage addition:**

```solidity
struct RouterDescriptor {
    bytes4    selector;
    uint8     tokenInputArgIndex;     // which calldata argument position is the input token
    bool      registered;
}
mapping(address router => mapping(bytes4 selector => RouterDescriptor)) routerSelectors;

function registerRouterSelector(
    address router,
    bytes4 selector,
    uint8 tokenInputArgIndex,
    /* standard gov sig */
) external;

function unregisterRouterSelector(
    address router,
    bytes4 selector,
    /* standard gov sig */
) external;
```

This lets the Stablecoin DEX, future MagnusBridge contracts, and any other routers register their selectors so naive wallets calling them have fees inferred correctly.

**New error:**

```solidity
error FeeTokenNotInferable();
```

If inference fails (e.g. unknown contract call with no router selector registered), the user tx reverts with this error. Wallet sees a clear signal that this kind of tx requires explicit `feeToken` specification.

#### Layer 2 — `@magnus/wallet-sdk` reference implementation (30 days post-launch)

Built by Magnus team. Optional drop-in for wallet developers and DApps that want richer UX. **Wallets work without the SDK** — the SDK improves UX, doesn't enable functionality.

```typescript
import { MagnusProvider, MagnusSigner } from '@magnus/wallet-sdk';

const provider = new MagnusProvider('https://rpc.magnus.xyz');
const signer = MagnusSigner.fromEthersSigner(ethersSigner);

// Standard ethers-style call — SDK pre-validates before submission
await signer.transferToken(USDT_ADDRESS, friend, 100);
// Behind the scenes:
//   - mirror inference rules (preview which token will be used for fees)
//   - balance pre-check: balanceOf(user, USDT) >= transferAmount + estimatedFee
//   - validator-acceptance pre-check: isAcceptedByAnyValidator(USDT)
//   - clear error if pre-checks fail (before user signs)
```

**SDK value-add:**

- **Pre-flight balance check** — wallet shows "you don't have enough USDT to send + fee" before user signs.
- **Pre-flight validator-acceptance check** — wallet shows "no validators currently accept this token; switch?" warning.
- **Multi-token UX hints** — "You're sending USDT but holding more vndBank; want to override fees to vndBank instead?"
- **Tx parsing for display** — user-friendly summaries.
- **Error decoding** — translate `FeeTokenNotInferable`, `FeeTokenNotAccepted`, etc. into actionable messages.

**Without the SDK, wallets still function.** Users may submit txs that revert with `FeeTokenNotInferable` for unrecognized router calls — they then learn to explicitly specify `feeToken` (or wait for the wallet to integrate the SDK). Direct MIP-20 transfers (the dominant case) work without the SDK.

**Mobile SDKs:** `@magnus/wallet-sdk-swift` (iOS) and `@magnus/wallet-sdk-kotlin` (Android) built by Magnus team in parallel; v0.1 within 90 days post-launch.

**Standard publication:**

A wallet-developer-facing **Wallet Fee-Token Inference Specification** is published at `docs.magnus.xyz/protocol/wallet-fee-token-inference` with mainnet launch (formal MIP designation TBD when Magnus's MIP process is stood up). The doc defines:

- Inference rules implemented by the protocol (canonical reference).
- ABI for `routerSelectors` registry.
- Reference test vectors (canonical tx → expected inferred fee token).
- Wallet developer integration guide for the optional SDK.

#### Behavior matrix per wallet sophistication

| Wallet behavior | Result |
|---|---|
| Vanilla EVM wallet (MetaMask, Trust, etc.), submits standard tx with no `feeToken` | **Works** for direct MIP-20 transfers and registered routers (auto-inferred). Reverts with `FeeTokenNotInferable` for unrecognized contract calls. |
| Wallet that explicitly sets `feeToken` in envelope | **Works** in all cases; protocol respects the override. |
| Wallet integrating `@magnus/wallet-sdk` | **Works**, plus pre-flight validation, plus richer UX warnings. |
| Sponsored tx / paymaster | **Works** with explicit `feeToken` set by sponsor. |

#### Code additions to FeeManager

- `infer_fee_token(tx)` helper invoked during `collect_fee_pre_tx`.
- `routerSelectors` storage map.
- `registerRouterSelector` and `unregisterRouterSelector` governance functions (standard 5-of-9).
- `FeeTokenNotInferable` error.
- Tests covering: direct MIP-20 transfer (auto-inferred), unrecognized contract call (revert), wallet-specified `feeToken` (respected), router selector match (inferred from calldata).

#### Genesis state

Pre-register selectors for known routers at genesis:

| Router | Selectors registered | Notes |
|---|---|---|
| Stablecoin DEX (`0xdec0…`) | `swapExactAmountIn(tokenIn,...)`, `swapExactAmountOut(tokenIn,...)` | tokenIn at arg index 0 |
| MIP-20 Factory (`0x20FC…`) | `createToken(...)` | Special case — newly created token doesn't exist yet; protocol falls back to `FeeTokenNotInferable`; wallet must specify `feeToken` for token creation txs |
| Future MagnusBridge | TBD per MBS spec | Register at MBS hardfork activation |

Future routers register via governance after deployment.

#### Out of scope for §11.3

- ERC-4337 account abstraction. Bigger architectural commitment; defer.
- WalletConnect protocol-level changes. SDK supports WC out of the box if added; no protocol changes needed.
- Wallet team formal partnerships. Adoption work, not engineering.
- Magnus public RPC middleware. With protocol-side inference, RPC doesn't need to inject `feeToken` — happens at execution time. RPC may still enrich `eth_estimateGas` responses for UX, but that's optional and not on the critical path.

## 12. Testing plan

- **Unit:** `IssuerRegistry.addApprovedIssuer` / `removeApprovedIssuer` — gov sig validation, currency check, replay protection, expiry.
- **Unit:** Factory rejects deploy from non-approved issuer with `IssuerNotApproved`.
- **Unit:** Factory accepts deploy from approved issuer; rejects USDC-deploy from address approved only for VND.
- **Unit:** `addAcceptedToken` / `removeAcceptedToken` for any registered+enabled MIP-20.
- **Unit:** `settle_fee` direct-credits when validator accepts; reverts with `FeeTokenNotAccepted` otherwise.
- **Integration:** Multi-validator devnet — Foundation accepts `{MagnusUSD, USDT, vndBank}`. Various users pay in various tokens; verify direct credit + correct revert behavior.
- **Integration:** Genesis-only deploy of MagnusUSD; verify reserved-address path bypasses IssuerRegistry.
- **Fuzz:** Random `(approved-issuer, currency, factory-call)` combinations. Assert: only approved-issuer + matching-currency-in-approval succeeds.
- **Gas:** Fee collection (direct credit) ≤ 8k gas per tx.

## 13. Future enhancements (out of v3.8.2 scope)

1. **Wallet inference normative spec** (depends on #11.3 decision).
2. **DEX-based cross-currency fee routing** if traffic patterns prove the need.
3. **Per-user fee-token preferences** if a UX requirement emerges that wallet-side inference can't handle.
4. **Per-token rate limits per validator** for treasury hygiene.
5. **Issuer reputation / scoring** — beyond binary approval, a reputation system for tracking issuer behavior over time.
6. **Multi-region validator distribution** — formal protocol-level region tagging.

## 14. Change log

- **2026-04-27 (v3.8.2):** Moved `MagnusUSD` from inherited Tempo `MagnusUSD` slot `0x20C0…0000` to fresh reserved slot `0x20C0…0010`. Slot `0x20C0…0000` is left empty in Magnus genesis (clean break from inherited Tempo state). Slot-0 special-cases in factory code (`mip20_factory/mod.rs:197`) generalized to the first-of-currency rule from §4.4a. Reserved-range allocation policy documented in §9.8: `0x000-0x00F` free, `0x010` MagnusUSD, `0x011-0x0FF` reserved for Magnus Foundation stablecoin family, `0x100-0x3FF` reserved for protocol use.
- **2026-04-26 (v3.8.1):** Removed placeholder "MIP-2010" identifier from §11.3. Replaced with neutral reference to a wallet-developer doc at `docs.magnus.xyz/protocol/wallet-fee-token-inference`. Formal MIP designation deferred until Magnus's MIP process is formally stood up.
- **2026-04-26 (v3.8):** Refined §11.3. Two-layer strategy (protocol-side inference + optional SDK), removed RPC middleware as a distinct layer. Protocol owns inference logic as the canonical reference. SDK is UX enhancement, not functionality requirement. Vanilla EVM wallets work for direct MIP-20 transfers + registered routers; require explicit `feeToken` override for unknown contract calls. `feeToken` envelope field retained for power-user / sponsor / paymaster use cases.
- **2026-04-26 (v3.7):** Initial §11.3 resolution with three-layer (protocol + SDK + RPC middleware) strategy. Refined in v3.8.
- **2026-04-26 (v3.6):** Resolved §11.1 disable-currency semantics. Hybrid mechanism: standard `disableCurrency` with grace period (Option C) + `emergencyDisableCurrency` immediate hard-stop (Option A) + `pruneCurrency` for state cleanup (Option B). Both grace duration and emergency threshold are governance-tunable with sanity bounds. Genesis defaults: 30-day grace, 7-of-9 emergency threshold. Resolved §11.2 validator-org churn. Hybrid distribute-on-removal with escrow fallback (Option D). New `offboardValidator` + `claimEscrowedFees` + `sweepExpiredEscrow` functions. Bounded-gas low-level call with USDT-compatible void-return handling. Genesis default: 365-day claim window.
- **2026-04-26 (v3.5):** Issuer allowlist (`IMIP20IssuerRegistry` precompile, per-currency, multisig-only) added as v1 scope. Magnus Foundation deploys real `MagnusUSD` (renamed from inherited `MagnusUSD`) as bootstrap stablecoin issuer. Multi-token validator accept-sets retained from v3.4. Wallet-inferred fee token retained from v3.4. Fee AMM deletion confirmed. Launch plan: USD + VND from day 1, Magnus Foundation as sole launch validator-org, ~6-week delay.
- **2026-04-26 (v3.4):** Multi-token validator accept-set, removed `userTokens`, removed Fee AMM from fee path.
- **2026-04-26 (v3.2):** Validator-token bootstrap (§4.5a). Removed `MAGNUS_USD_ADDRESS` fallback.
- **2026-04-26 (v3.1):** First-of-currency bootstrap (§4.4a). Factory accepts `quoteToken = 0`.
- **2026-04-26 (v3):** Validators as partner organizations. Removed DEX from fee path.
- **2026-04-26 (v2):** Issuer-first, no-canonical-token model. Removed path-token references.
- **2026-04-24 (v1):** Initial draft proposing path-token family extending Tempo's `MagnusUSD`.
