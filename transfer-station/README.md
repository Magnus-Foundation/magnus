# Magnus

The stablecoin wallet. Send anywhere. Zero gas. Always.

## What it does

Your stablecoins live on Magnus. Every fee is in stablecoins. No gas tokens, ever.

- **Deposit from any chain** — Sign a message. USDT appears. No ETH/SOL/TRX needed.
- **Send to any chain** — Ethereum, Solana, Tron, BNB, Base. Zero gas on destination.
- **Send to any bank** — VietQR, M-Pesa, GCash, UPI, PIX. One transaction, 300ms.
- **Pay in stablecoins only** — Every fee is USDT/USDC. No gas tokens. Ever.

## Why this exists

You have 100 USDT on Ethereum. You can't send it without buying ETH first. On Tron, you need TRX. On Solana, SOL. 125 chains, 125 gas tokens. The #1 UX failure in crypto.

Magnus fixes this. One wallet, stablecoin fees only, routes to 125 chains and 195 countries.

## How zero gas works

**Deposit (any chain → Magnus):** User signs an off-chain message (free). A relayer submits the transaction and pays gas. Relayer is reimbursed in stablecoins from the user's deposit. User never touches gas tokens.

**Send (Magnus → any chain):** User pays 0.05 USDT. A netting engine matches inbound and outbound flows. 90% of transfers cancel out, never crossing a bridge. The protocol pays destination gas from settlement fees for the 10% that do bridge.

**Internal (Magnus → Magnus):** 300ms, ~0.001 USDT. No bridge, no gas token.

## Architecture

```
         125 CHAINS                    195 COUNTRIES
    ┌── Ethereum                  VietQR (Vietnam) ──┐
    ├── Solana                    M-Pesa (Kenya) ────┤
    ├── Tron          ┌──────┐   GCash (Philippines) ┤
    ├── BNB     ──MBS─┤MAGNUS├─MGP── UPI (India) ────┤
    ├── Base          │  L1  │   PIX (Brazil) ───────┤
    ├── Arbitrum      └──────┘   ACH/SEPA (US/EU) ───┘
    └── ...125 more
    
    MBS = Magnus Bridge Standard (one Solidity template, deploy on any chain)
    MGP = Magnus Gateway Protocol (fiat rail integration standard)
```

## Two open standards

**Magnus Bridge Standard (MBS)** — How any chain connects to Magnus. One Solidity template (~500 LOC). Deploy it on your chain. Connected. Gas-free deposits via ERC-2612 permit + relayer.

**Magnus Gateway Protocol (MGP)** — How any fiat rail connects to Magnus. Run the gateway daemon. Your users can receive fiat from Magnus. On-chain escrow, settlement attestation, gateway registry.

The standards are open. The moat is the network.

## Competitive position

| Feature | Stellar | Arc (Circle) | Magnus (Stripe) | Magnus |
|---|---|---|---|---|
| Gas-free deposits | No | No | No | Yes (permit + relayer) |
| Gas-free sends | No | No | No | Yes (netting + treasury) |
| Fiat rails | App-layer (SEP) | No | No | Native (MGP) |
| Netting | No | No | No | Yes (90%+ target) |
| EVM | No (Soroban) | No | Yes (Reth) | Yes (revm) |
| Finality | 3-5s | Sub-second | Sub-second | 300ms |

## Phased build

- **Phase 0** (May 2026): DEX + simulated Gateway + demo wallet on magnus-chain (Magnus fork).
- **Phase 1** (July 2026): Gas-free deposits (permit + relayer). Real Gateway partner. Gateway SDK.
- **Phase 2** (Q3 2026): MagnusBridge.sol template. Bridges to Ethereum + Tron. Gas-free sends.
- **Phase 3** (Q4 2026): Netting engine. Compliance pipeline. Open relayer market.

## Links

- [Full Design Doc](design.md)
- [Alliance DAO Application](alliance-dao-application.md)
- [Payment Engine Research](payment-engine-research.md)
- [Pitch Deck Prompt](pitch-deck-prompt.md)
