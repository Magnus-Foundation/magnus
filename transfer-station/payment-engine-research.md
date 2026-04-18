# Research Report: Payment Engine Architecture for Magnus

## Executive Summary

Five production systems and 40+ academic papers analyzed. Key finding: every design decision in the Magnus implementation plan has prior art to validate it — and several critical improvements to make based on what we found.

The most important discovery: **Tempo uses end-of-block order matching**, not continuous per-transaction matching. This solves the MEV/front-running problem and reduces chain load dramatically. The Magnus order book should match at end-of-block, not on every PlaceOrder call.

The second most important discovery: **Codex's OnrampTx/OfframpTx are native transaction types that atomically combine value transfer + compliance + fiat settlement instructions**. This is architecturally superior to our current GatewayWithdraw design and should inform Phase 1.

The third: **XRPL's DirectoryNode encoding** (256-bit key with pair in upper bits, price in lower bits) enables O(log n) order book traversal without sorting. We should adopt a similar encoding for our storage slot key computation.

---

## Key Findings

### 1. PathPayment Atomicity: Stellar's Approach is the Gold Standard

Stellar's `LedgerTxn` (database transaction wrapper) gives total atomicity for PathPayment:
- All hop state changes are buffered
- If any hop fails: entire transaction rolled back
- Sequence number and fee consumed even on failure (replay prevention)
- No partial state ever committed

**For Magnus:** Our current `execute_path_payment` rollback implementation is correct. The snapshot + restore pattern mirrors what Stellar does. One refinement: fees should be consumed even on failure (our current implementation returns an error and consumes no gas — this is a correctness issue for DOS prevention).

**CAP-0004 Rounding Algorithm:** Stellar's exchange algorithm prevents crossed order books by always removing the lower-value offer. Our Phase 0 order book doesn't need this complexity (demo), but Phase 1 must implement bounded rounding.

### 2. Order Book Matching: Match at End-of-Block (Tempo Pattern)

The most actionable finding from the stablechain research:

**Tempo's DEX:** Orders accumulate during block execution. At the end of the block, a system call settles all accumulated orders. This means:
- Individual PlaceOrder transactions just add to the queue
- No per-transaction matching overhead
- All matching happens once per block
- Eliminates MEV/front-running on order placement (no sandwiching)
- Reduces chain state writes dramatically

**Current plan problem:** Our `PlaceOrder` operation immediately tries to match against existing orders. This means every PlaceOrder is an expensive database read + potential write. End-of-block matching is strictly better.

**Fix for implementation plan:** Separate PlaceOrder (add to queue) from matching (end-of-block system call). The `PaymentEngine::finalize_block()` method runs matching after all transactions are processed.

### 3. XRPL MPT: Token Capability Flags — Reference Implementation

XRPL MPT (live mainnet October 2025) is exactly what our Token Registry should implement:

```
Capability flags (set immutably at token creation):
  lsfMPTCanLock       → circuit breaker / emergency freeze
  lsfMPTRequireAuth   → whitelist-only transfers
  lsfMPTCanEscrow     → gateway escrow support
  lsfMPTCanTrade      → DEX listing permission
  lsfMPTCanTransfer   → peer-to-peer transfer permission
  lsfMPTCanClawback   → issuer clawback right

Storage efficiency: ~88 bytes/holder vs. 244 for trust lines (2.4x smaller)
```

**For Magnus Phase 1:** Our Token Registry design (from design doc) should adopt this exact flag structure. XRPL MPT is the reference implementation we should study and extend.

### 4. XRPL DirectoryNode: Better Order Book Key Encoding

XRPL's 256-bit order book key encoding:
- Upper 192 bits: SHA-512Half(sell_currency, sell_issuer, buy_currency, buy_issuer)
- Lower 64 bits: price ratio in XRPL's 64-bit fixed-point format

This enables **ascending key traversal = best-to-worst price traversal** with no sorting. Our current implementation uses `keccak256("order_book" || sell_asset || buy_asset)` which is a flat key — we serialize the entire BTreeMap to one slot.

**For Phase 1 migration to per-order slots:** Use a 256-bit key similar to XRPL's DirectoryNode. Pack the trading pair in the high bits and price in the low bits. BTreeMap<U256, Vec<Order>> over storage slots gives O(log n) price-level traversal.

### 5. CLS Bank Netting: Minimum Viable Implementation

The 96% netting efficiency comes from multilateral netting: compute net position per (member, currency) = sum(inflows) - sum(outflows). This is O(V+E) where V = participants and E = payment instructions.

**Simple algorithm for Phase 3:**
```
every N blocks:
  for each (participant, asset) pair:
    net_position = sum(pending_receives) - sum(pending_sends)
    if net_position > 0: credit participant
    if net_position < 0: debit participant for bridge settlement
  clear all pending cross-chain instructions
  only participants with net negative positions need to fund via bridges
```

**Cycles Protocol** (Buchman, arXiv 2507.22309) is the most relevant blockchain implementation — ZK-TEE hybrid for privacy-preserving multilateral netting. Read this paper before implementing the netting engine.

**Key insight from CLS:** The Positive Account Balance Rule is the invariant. A participant can have a negative position in one asset as long as their aggregate portfolio (USD-equivalent) remains non-negative. Our netting engine should enforce this multi-asset portfolio balance check, not per-asset checks.

### 6. Codex Gateway Design: Economic Assurance Network

Codex's OnrampTx/OfframpTx atomically combine:
1. Value transfer (debit sender)
2. Rules evaluation (compliance check)
3. Fiat settlement instructions (emit event to Gateway)

If rules fail → atomic revert. The Economic Assurance Network slashes gateway validators for non-delivery.

**For Magnus Phase 1:** 
- GatewayWithdraw should be a native transaction type that atomically: debits user, escrows funds, emits settlement instruction
- Gateway settlement must produce an on-chain attestation or the escrow is clawed back (slashing)
- Phase 0 simulates this; Phase 1 should implement EAN-style slashing

### 7. Stellar SEP vs. Our Gateway Protocol

The research confirms our competitive positioning:
- Stellar anchors are application-layer: the chain cannot verify fiat delivery, no protocol-native escrow
- Our Gateway operations are chain-native: escrow enforced by protocol, settlement attestation on-chain, circuit breaker applies
- SEP-31 relies on memo matching and trust-based callbacks; our Gateway uses on-chain event verification

**Confirmed: our Gateway Protocol is architecturally superior to Stellar anchors.**

### 8. Arc's StableFX: Off-Chain RFQ + On-Chain PvP

Arc's FX engine uses off-chain RFQ for price discovery (not an on-chain order book). This is the institutional design. For Phase 0 we use an on-chain order book (simpler, more transparent), but Phase 2+ should support an off-chain RFQ mode for large institutional trades.

---

## Implementation Changes Based on Research

### Critical: Add end-of-block matching to PaymentEngine

```rust
impl PaymentEngine {
    /// Called after all transactions in a block are processed.
    /// Matches accumulated orders end-of-block (Tempo pattern).
    /// Returns ChangeSet including matched order state.
    pub fn finalize_block(&mut self) -> ChangeSet {
        // 1. Match orders in USDT/VND book
        self.usdt_vnd_book.match_accumulated_orders(&mut self.balances);
        // 2. Convert all state to ChangeSet
        self.to_changeset()
    }
}
```

The `OrderBook::match_accumulated_orders()` method processes all accumulated PlaceOrder operations at once, in price-time order, end-of-block.

### Critical: Fees consumed even on failure

```rust
// In composite.rs, payment execution failure:
Err(e) => {
    let gas_used = 5000u64;  // Fee consumed even on failure
    cumulative_gas = cumulative_gas.saturating_add(gas_used);
    let receipt = ExecutionReceipt::new(
        tx_hash, false, gas_used, cumulative_gas, vec![], None,
    );
    outcome.receipts.push(receipt);
    // State NOT applied — rollback is automatic (engine didn't commit)
    tracing::warn!(?e, "payment execution failed");
}
```

### Nice to have: Separate order queuing from matching

```rust
pub enum PaymentOp {
    // ...existing...
    PlaceOrder {
        sell_asset: AssetId,
        buy_asset: AssetId,
        sell_amount: u128,
        price: u128,
    },
    // Phase 1 addition:
    CancelOrder {
        order_id: u64,
    },
}
```

PlaceOrder adds to a queue during block execution. `finalize_block()` matches the queue.

---

## Sources

### Stellar
- PathPaymentStrictSendOpFrame.cpp — stellar-core (source code)
- CAP-0004: exchangeV10 rounding algorithm
- CAP-0037/CAP-0038: AMM integration with CLOB for path payments
- SEP-0006/24/31: Anchor protocol specifications

### CLS Bank
- BIS Working Paper 1318: Auction-based liquidity saving mechanisms
- CPMI 2022: Facilitating increased adoption of PvP
- Gavrila & Popa (2021): Novel algorithm for clearing financial obligations
- Cycles Protocol (Buchman, arXiv 2507.22309): Decentralized multilateral netting

### XRPL
- XLS-33: Multi-Purpose Tokens specification (MPT, live Oct 2025)
- XRPL DirectoryNode documentation
- XLS-56d: Atomic/Batch Transactions (upcoming)

### Academic
- Malavolta et al., NDSS 2019: Anonymous Multi-Hop Locks (AMHLs)
- Sivaraman et al., NSDI 2020: Spider packet-switched routing
- BIS WP 1335 (Shin, March 2026): Tokenomics and blockchain fragmentation equilibrium
- arXiv 2601.00196 (Jan 2026): SoK: Stablecoins in Retail Payments (CLEAR framework)
- Lighter Protocol Whitepaper: ZK-SNARK on-chain CLOB feasibility
- Cao-Yuan, FC 2020: Decentralized privacy-preserving netting on blockchain

### Stablechain Architectures
- Tempo GitHub + docs.tempo.xyz: TIP-20, Enshrined DEX, MPP, end-of-block matching
- Codex: OnrampTx/OfframpTx, EAN slashing, compliance precompiles
- Arc: Malachite consensus, StableFX RFQ+PvP, CPN integration
- Commonware: threshold-simplex, QMDB, p2p::authenticated

---

*Research conducted 2026-04-17/18. 5 parallel subagents. 40+ primary sources.*
