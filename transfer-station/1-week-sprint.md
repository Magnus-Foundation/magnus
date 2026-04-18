# Magnus — 1-Week Demo Sprint (magnus-chain base)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Working demo in 7 days. "USDT in, VND out, 300ms, one transaction."

**Base:** magnus-chain (Magnus fork with MIP20 tokens, StablecoinDEX, MIP403 compliance, MipFeeManager, Reth EVM, Commonware Simplex consensus). Already has 80% of what we need.

**What we ADD:**
- Gateway precompile (~300 LOC Rust)
- Genesis config (deploy USDT + VND tokens, seed DEX, register gateway)
- Demo wallet (HTML/JS, standard ethers.js, MetaMask compatible)

**Demo script (what the audience sees):**
1. Open wallet. See 10,000 USDT balance.
2. Click "Send to Vietnam." Enter amount: 100 USDT.
3. Under the hood: DEX swap USDT→VND + Gateway withdraw → VietQR reference
4. Screen shows: "Confirmed in 312ms. 2,500,000 VND delivered via VietQR."
5. Balance updates: 9,900 USDT.

---

## Existing Infrastructure (from Magnus fork)

Already built and working in magnus-chain. We use these as-is:

```
✓ Commonware Simplex consensus    (300ms finality)
✓ Reth EVM                        (full Solidity compatibility)
✓ MIP20 tokens                    (ERC-20 precompile, memo, ISO 20022, currency, policy)
✓ MIP20Factory                    (deploy new tokens)
✓ StablecoinDEX                   (order book, limit orders, flip orders, swap)
✓ MIP403 Registry                 (compliance whitelist/blacklist)
✓ MipFeeManager                   (multi-stablecoin gas fees)
✓ AccountKeychain                 (key management)
✓ QMDB state backend              (parallel merkleization)
✓ Indexer                         (block/tx/receipt indexing)
✓ RPC server                     (Ethereum JSON-RPC compatible)
✓ Transaction pool               (nonce ordering, fee prioritization)
✓ DKG                            (distributed key generation)
```

## Day-by-Day Plan

### Day 1: Compile + Run + Verify Existing Infrastructure

**Morning:**

- [ ] Verify `cargo check` passes (compiling now)
- [ ] Run `cargo run --bin magnus -- --help` and understand CLI options
- [ ] Find genesis/chainspec configuration
- [ ] Start a single-validator local devnet
- [ ] Verify blocks are produced

**Afternoon:**

- [ ] Find MIP20Factory deployment in genesis
- [ ] Query an existing MIP20 token via `eth_call` (balanceOf)
- [ ] Submit a basic EVM transaction via `eth_sendRawTransaction`
- [ ] Verify StablecoinDEX precompile is accessible

**Exit criteria:** Node runs. Blocks produced. Can query precompiles via JSON-RPC.

---

### Day 2: Genesis Config — Deploy USDT + VND + Seed DEX

**Files to modify:**
- Genesis/chainspec config (find via `grep -r "genesis\|Genesis\|chainspec" crates/ bin/`)
- Possibly `crates/core/chainspec/`

**Tasks:**

- [ ] Deploy MIP20 USDT token via MIP20Factory in genesis
  - symbol: "USDT", currency: "USD", decimals: 6
  - Mint 10,000 USDT to Alice (demo account)
  - Mint 1,000,000 USDT to MarketMaker

- [ ] Deploy MIP20 VND token via MIP20Factory in genesis
  - symbol: "VND", currency: "VND", decimals: 6
  - Mint 25,000,000,000 VND to MarketMaker

- [ ] Create StablecoinDEX pair (USDT/VND)
  - `StablecoinDEX.createPair(USDT_ADDRESS)`

- [ ] Seed DEX with market maker liquidity
  - MarketMaker places sell orders: 1,000,000 USDT at price tick 25,000
  - MarketMaker places flip orders (auto-replenish)

- [ ] Verify via eth_call:
  - `MIP20_USDT.balanceOf(Alice)` = 10,000
  - `StablecoinDEX.quoteSwapExactAmountIn(USDT, VND, 100)` returns ~2,500,000

**Exit criteria:** Tokens deployed. Alice has USDT. DEX has liquidity. Swap quotes work.

---

### Day 3: Build Gateway Precompile

**Files to create:**
- `crates/precompile/contracts/src/precompiles/gateway.rs`
- `crates/precompile/registry/src/gateway/mod.rs`
- `crates/precompile/registry/src/gateway/dispatch.rs`

**Files to modify:**
- `crates/precompile/contracts/src/precompiles/mod.rs` (add gateway)
- `crates/precompile/registry/src/lib.rs` (register gateway precompile)

**Gateway precompile interface (Solidity ABI):**

```solidity
interface IMagnusGateway {
    // Register a new gateway (governance-gated in production, open in demo)
    function registerGateway(
        string calldata name,
        string[] calldata supportedRails
    ) external returns (uint32 gatewayId);

    // Gateway attests fiat received, credits stablecoin to user
    function deposit(
        uint32 gatewayId,
        address recipient,
        address token,          // MIP20 token address
        uint256 amount,
        bytes calldata proof    // fiat reference (not validated in Phase 0)
    ) external;

    // User sends stablecoin to gateway, receives fiat rail reference
    function withdraw(
        uint32 gatewayId,
        address token,
        uint256 amount,
        string calldata fiatRail,       // "vietqr"
        string calldata fiatRecipient   // "VN-1234567890"
    ) external;

    // Events
    event GatewayRegistered(uint32 indexed gatewayId, string name);
    event GatewayDeposit(uint32 indexed gatewayId, address indexed recipient, address token, uint256 amount);
    event GatewayWithdraw(
        uint32 indexed gatewayId,
        address indexed sender,
        address token,
        uint256 amount,
        string fiatRail,
        string fiatRecipient,
        string reference       // "SIMULATED-vietqr-VN-123-2500000"
    );
}
```

**Implementation pattern (follow MIP20 precompile pattern):**

```rust
// In registry/src/gateway/mod.rs
// Follow the exact pattern from registry/src/mip20/mod.rs:
// 1. Parse calldata using alloy sol! macro ABI
// 2. Use StorageCtx for EVM state reads/writes
// 3. Call MIP20 token.transfer/mint/burn for balance changes
// 4. Return PrecompileResult with gas cost + output
```

**Tasks:**

- [ ] Define Gateway interface in `contracts/src/precompiles/gateway.rs`
- [ ] Implement Gateway storage (registry of gateways in EVM slots)
- [ ] Implement `deposit()`: call MIP20.mint(recipient, amount) on the token
- [ ] Implement `withdraw()`: call MIP20.burn(sender, amount), emit GatewayWithdraw event with simulated reference
- [ ] Register at fixed address `GATEWAY_ADDRESS` in `extend_magnus_precompiles()`
- [ ] Register demo gateway in genesis (ID=1, "Demo VietQR Gateway", rails=["vietqr"])

**Exit criteria:** `cargo check` passes. Gateway precompile registered. Can call `gateway.withdraw()` via eth_sendTransaction and see the GatewayWithdraw event in the receipt.

---

### Day 4: Demo Wallet

**Files to create:**
- `demo-wallet/index.html` (single file, embedded JS/CSS)

**The wallet uses standard Ethereum tooling:**

```javascript
// ethers.js from CDN — standard Ethereum library
// Connects to local Magnus node via JSON-RPC
// Uses standard ERC-20 ABI for MIP20 tokens (compatible)
// Uses StablecoinDEX ABI for swap
// Uses Gateway ABI for withdraw

const provider = new ethers.JsonRpcProvider("http://localhost:8545");
const wallet = new ethers.Wallet(DEMO_PRIVATE_KEY, provider);

// Read balance (standard ERC-20 balanceOf)
const balance = await usdt.balanceOf(wallet.address);

// Swap USDT → VND (StablecoinDEX)
await usdt.approve(DEX_ADDRESS, amount);
await dex.swapExactAmountIn(USDT_ADDR, VND_ADDR, amount, minOut);

// Withdraw VND → VietQR (Gateway)
await vnd.approve(GATEWAY_ADDRESS, vndAmount);
await gateway.withdraw(1, VND_ADDR, vndAmount, "vietqr", "VN-123");
```

**Wallet features:**
- Dark theme, teal accents, "Magnus" branding
- Balance display: USDT + VND (auto-refresh)
- "Send to Vietnam" button: approve → swap → withdraw in sequence
- Transaction confirmation with timing ("Confirmed in 312ms")
- Transaction history (from receipts)
- "SIMULATED" labels on mock elements
- MetaMask compatible (can also use MetaMask instead of embedded key)

**Exit criteria:** Open in browser. See balances. Execute the full flow.

---

### Day 5: Full Flow Integration + Polish

**Morning:**

- [ ] Run the complete demo flow 5 times end-to-end:
  1. Open wallet, see 10,000 USDT
  2. Click "Send to Vietnam" with 100 USDT
  3. Approve USDT → DEX swap → approve VND → Gateway withdraw
  4. See "Confirmed in Xms"
  5. Balance updates to 9,900 USDT

- [ ] Fix any bugs found

- [ ] Time the flow — is it under 1 second total? (should be ~300ms per tx, 3 txs ≈ 1 second)

**Afternoon:**

- [ ] Optimize: can we batch approve+swap+withdraw into fewer user clicks?
  - Option: Router contract that does approve+swap+gateway in one tx
  - Or: wallet pre-approves max amounts
  
- [ ] Add "simple transfer" mode (Alice → Bob USDT, no swap/gateway)

- [ ] Test edge cases:
  - Amount > balance
  - Zero amount
  - DEX with insufficient liquidity
  - Node restart (state persistence)

**Exit criteria:** Demo works 5/5 times. Timing is under 2 seconds for the full flow.

---

### Day 6-7: Demo Script + Video + Buffer

- [ ] Write the demo script (what to click, what to say)
- [ ] Record video backup (screen capture with voiceover)
- [ ] Alliance DAO application final review (add demo link placeholder)
- [ ] Buffer for any remaining bugs

---

## Architecture (what's existing vs new)

```
┌────────────────────────────────────────────────────────────┐
│  DEMO WALLET (Day 4)                                       │
│  ethers.js → MetaMask compatible → standard ERC-20/ABI     │
└────────────────────┬───────────────────────────────────────┘
                     │ JSON-RPC (eth_sendTransaction, eth_call)
                     ▼
┌────────────────────────────────────────────────────────────┐
│  MAGNUS NODE (existing, magnus-chain)                      │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Precompiles (revm)                                  │  │
│  │                                                      │  │
│  │  EXISTING:           │  NEW (Day 3):                 │  │
│  │  ✓ MIP20 (ERC-20)    │  ✦ Gateway precompile        │  │
│  │  ✓ MIP20Factory      │    registerGateway()          │  │
│  │  ✓ StablecoinDEX     │    deposit()                  │  │
│  │  ✓ MIP403 Registry   │    withdraw()                 │  │
│  │  ✓ MipFeeManager     │    ~300 LOC                   │  │
│  │  ✓ AccountKeychain   │                               │  │
│  └──────────────────────┴───────────────────────────────┘  │
│                                                            │
│  EXISTING: Simplex consensus, QMDB, Reth EVM, RPC, TxPool │
└────────────────────────────────────────────────────────────┘
```

## Risk Mitigation

**Risk 1: Compilation fails (Reth dependency issues)**
- Mitigation: If magnus-chain doesn't compile, fall back to kora + Solidity contracts (Option A from earlier discussion). 2-3 day pivot.

**Risk 2: Genesis config is complex (Reth chainspec)**
- Mitigation: Find how existing MIP20 tokens are deployed. Follow that pattern. Read `crates/core/chainspec/`.

**Risk 3: Gateway precompile integration breaks something**
- Mitigation: Follow exact pattern from existing precompiles (MIP20, StablecoinDEX). The `magnus_precompile!` macro and `StorageCtx` handle all the revm integration.

## What this sprint does NOT include

- Gas-free deposits via permit+relayer (Phase 1)
- Bridge contracts / MagnusBridge.sol (Phase 2)
- Netting engine (Phase 3)
- Polished wallet UI (Week 2)
- Multi-validator devnet (Week 2)
- E2E automated tests (Week 2)
