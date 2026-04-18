# Magnus Pitch Deck — Visual-First, 12 Slides

30% text, 70% visual. One idea per slide. Lead with user experience, not infrastructure.

```
STRUCTURE (12 slides):
  Slide 1      Title
  Slide 2-4    PROBLEM & PRODUCT (3 slides)
  Slide 5-6    HOW IT WORKS (2 slides)
  Slide 7-9    MARKET & COMPETITION (3 slides)
  Slide 10-11  BUSINESS MODEL & ROADMAP (2 slides)
  Slide 12     Close
```

---

## SLIDE 1: Title

**Text:** Magnus — One Wallet. Every Stablecoin. Every Chain. Every Bank Account.
**Visual:** Full-bleed dark gradient. Magnus logo center. Below: "No destination gas fees. 300ms finality. Your stablecoins go everywhere."

---

## — PROBLEM & PRODUCT (slides 2-4) —

## SLIDE 2: The Problem

**Text:** Moving stablecoins shouldn't require a PhD.
**Visual:** Side-by-side comparison, full slide:

```
TODAY: Send $100 USDT from US to Vietnam

Step 1: Buy TRX for gas                    5 min
Step 2: Bridge USDT from Ethereum to Tron   10 min + $3 bridge fee
Step 3: Send USDT to exchange               2 min + $1 TRX gas
Step 4: Convert USDT to VND on exchange     5 min + 0.5% spread
Step 5: Withdraw VND to bank via local rail 1-24 hours

5 steps. 3 platforms. 2 gas tokens. 1-24 hours.
───────────────────────────────────────────
TRANSFER STATION:

"Send $100 to this VietQR account"          300ms. Done.
1 step. 1 wallet. 0 gas tokens. $0.05 fee.
```

The contrast tells the story.

---

## SLIDE 3: The Product

**Text:** One wallet that goes everywhere.
**Visual:** Wallet UI mockup, center of slide. Clean, minimal.

```
  ┌─────────────────────────────┐
  │  My Wallet                  │
  │                             │
  │  USDT      500.00           │
  │  USDC      200.00           │
  │  VND    5,000,000           │
  │                             │
  │  [Send to chain ▾]          │  → Ethereum, Solana, Tron, BNB
  │  [Send to bank  ▾]          │  → VietQR, M-Pesa, UPI, PIX
  │  [Receive       ]           │  → one address, any source
  └─────────────────────────────┘
```

Three bullet points below:
- **Any chain:** Ethereum, Solana, Tron, BNB, Base. Zero gas on destination.
- **Any bank:** VietQR, M-Pesa, GCash, UPI, PIX. 300ms to fiat.
- **Any stablecoin:** USDT, USDC, DAI, PYUSD. One balance, one address.

---

## SLIDE 4: Why It's Possible Now

**Text:** Three things changed in 2025-2026.
**Visual:** Three panels, each with one giant number:

```
$7.2T           $1.8B           5
monthly         Mastercard      licensed
stablecoin      acquired        exchanges
volume          BVNK            in Vietnam
(Feb 2026,      (proving the    (Decision 96,
overtook ACH)   market pays)    Jan 2026)
```

Bottom: "Stablecoins won. The infrastructure to use them hasn't caught up."

---

## — HOW IT WORKS (slides 5-6) —

## SLIDE 5: The Architecture

**Text:** Hub and spoke. One hub connects every chain and every bank.
**Visual:** Hub diagram, full slide:

```
        Ethereum ────┐
                     │
        Solana ──────┤
                     │
        Tron ────────┤     ┌── VietQR (Vietnam)
                     │     │
        BNB ─────────┼─ TS ┼── M-Pesa (Kenya)
                     │     │
        Base ────────┤     ├── UPI (India)
                     │     │
        Arbitrum ────┘     └── PIX (Brazil)
                     
              CHAINS            BANKS
```

Center label: "Magnus — Payment Engine + Gateway Protocol + Netting Engine"

Bottom: "Not 125 chains connected by broken bridges. One hub where everything settles."

---

## SLIDE 6: Zero Gas Fees — How

**Text:** 90% of transfers never cross a bridge.
**Visual:** Flow diagram showing netting:

```
INCOMING                              OUTGOING
100 USDT from Ethereum ──┐    ┌──→ 80 USDT to Ethereum
 50 USDT from Tron ──────┤    ├──→ 30 USDT to Tron
 30 USDT from Solana ────┘    └──→ 20 USDT to Solana
                         │    │
                    NETTING ENGINE
                    Net: 180 in, 130 out
                    Bridge needed: 0
                    (all netted against inflows)

Result: user pays $0.05 in USDT. No ETH. No SOL. No TRX.
Protocol absorbs destination gas from settlement fees.
```

Bottom: "CLS Bank nets 96% of $6.4T daily FX volume. Same principle, applied to stablecoins."

---

## — MARKET & COMPETITION (slides 7-9) —

## SLIDE 7: First Market — Vietnam

**Text:** $19B remittance corridor. 6.49% average fees. 5 new exchanges.
**Visual:** Vietnam corridor map with three giant numbers:

**$19B** /year US→Vietnam remittance
**6.49%** average fee (World Bank)
**5** newly licensed crypto exchanges (Jan 2026)

Arrow from US flag to Vietnam flag. Magnus logo on the arrow.

Small text: "These 5 exchanges need cross-border settlement rails. Nobody provides them. We do."

---

## SLIDE 8: Competition

**Text:** Everyone solves chain-to-chain. Nobody solves chain-to-bank.
**Visual:** 2x2 matrix, full slide:

```
                    CHAINS CONNECTED
                    Few          Many
              ┌──────────┬──────────────┐
   FIAT       │          │              │
   RAILS  No  │  Codex   │  Arc, Magnus  │
              │  (L2)    │  (stablechains)│
              ├──────────┼──────────────┤
              │          │              │
          Yes │  BVNK    │  Magnus ★    │
              │  (central│              │
              │  ized)   │              │
              └──────────┴──────────────┘
```

Bottom: "BVNK sold for $1.8B with centralized fiat rails. We're the decentralized, protocol-native version that connects to every chain."

Side note: "Stellar has fiat anchors but no EVM, no compliance, no netting. 10 years, same limitations."

---

## SLIDE 9: Why Not Stellar?

**Text:** Stellar proved the model. We're building the next generation.
**Visual:** Three-column comparison:

```
STELLAR (10 years)              MAGNUS (building now)

Anchors are app-layer      →   Gateways are protocol-native
  (SEP-6/24/31 standards)       (on-chain escrow, registration,
  Chain can't enforce.           settlement attestation)

No EVM                     →   Full EVM (revm)
  Soroban VM, isolated           Every token = ERC-20
  from $200B DeFi ecosystem      Full DeFi composability

No compliance, no netting  →   Protocol-level both
  App-layer only                 Whitelist, sanctions, freeze
                                 90%+ netting efficiency
```

Bottom: "Stellar is SMTP. We're building Gmail."

---

## — BUSINESS MODEL & ROADMAP (slides 10-11) —

## SLIDE 10: Revenue

**Text:** Basis points on every settlement. Gas-free UX funded by settlement fees.
**Visual:** Revenue stack:

```
REVENUE STREAMS:

1. Settlement fees         0.01-0.05% per transaction
   (on ALL volume)         → $3.6M/yr at $7.2B monthly volume

2. Multi-currency gas      Pay tx fees in any stablecoin
   (on-chain activity)     → Variable, scales with usage

3. Gateway licensing       Annual registration fee
   (per gateway partner)   → $10-50K per gateway

4. Premium netting         Priority settlement tier
   (institutional)         → Higher fee, instant settlement
```

Bottom comparison: "SWIFT: 2-7%. Visa: 1-3%. BVNK: 0.5%. Magnus: 0.01-0.05%."

---

## SLIDE 11: Roadmap

**Text:** Demo in 6 weeks. Kill criterion at month 3.
**Visual:** Horizontal timeline with milestones:

```
NOW ────── MAY 28 ────── JULY ────── Q3 ────── Q4 ────── 2027
 │            │            │          │          │          │
 Alliance     Unchained    Devnet     Bridges    Full       Mainnet
 DAO app      Summit       + real     ETH +      stack      beta
 submitted    Da Nang      Gateway    Tron       + EVM
              DEMO         partner               + netting
                                                 + compliance
              │
              ▼
         Phase 0: Payment Engine
         + PathPayment + FX
         + Simulated Gateway
         + Polished demo wallet

                           MONTH 3: ⚠ KILL CRITERION
                           No exchange partner running
                           test transactions = pivot
```

---

## SLIDE 12: Close

**Text:**

Stablecoins are on 125 chains.
Bank accounts are in 195 countries.

Magnus connects them all.
One wallet. 300ms. Zero gas fees.

**Visual:** Dark slide. Four lines. Large. Centered.

Below: "[Founder Name] | [email] | Pre-Seed | Alliance DAO ALL18"

---

## Pillar Balance Check

| Pillar | Slides | % |
|--------|--------|---|
| Problem & Product | 2, 3, 4 | 25% |
| How It Works | 5, 6 | 17% |
| Market & Competition | 7, 8, 9 | 25% |
| Business & Roadmap | 10, 11 | 17% |
| Title + Close | 1, 12 | 17% |

Balanced. Product-led, not infrastructure-led.

---

## Generation Prompt (copy-paste to Gamma / Beautiful.ai / Tome)

> Create a 12-slide pitch deck for Magnus, a stablecoin settlement chain that gives users one wallet for every stablecoin, every chain, and every bank account with zero destination gas fees.
>
> DESIGN RULES:
> - 30% text, 70% visual on every slide
> - One idea per slide
> - Scannable in 3 seconds
> - Lead with user experience, not infrastructure
> - The visual tells the story before text is read
>
> NARRATIVE ARC:
> "Moving stablecoins is broken" → "One wallet that goes everywhere" → "Here's how" → "Here's the market" → "Here's the business" → "Here's the plan"
>
> STRUCTURE:
> - Problem & Product (3 slides): today's 5-step pain vs 1-step Magnus, wallet mockup showing send-to-chain + send-to-bank, three market timing numbers ($7.2T, $1.8B BVNK, 5 VN exchanges)
> - How It Works (2 slides): hub-and-spoke diagram (chains on left, banks on right, TS in center), netting flow showing 90% of transfers never bridge + zero gas fee mechanism
> - Market & Competition (3 slides): Vietnam first market ($19B corridor, 5 exchanges), 2x2 matrix (chains x fiat rails, TS is only one in top-right), Stellar comparison (app-layer vs protocol-native, no EVM, no compliance)
> - Business & Roadmap (2 slides): revenue stack (settlement fees, gas, licensing, premium netting), timeline (May demo → July devnet → Q3 bridges → Q4 full stack, month-3 kill criterion)
> - Title + Close (2 slides)
>
> Style: Dark navy (#0a0e27). Teal/cyan (#06b6d4) accents. White text. Inter or SF Pro. No stock photos. Wallet mockups, hub diagrams, flow charts, and large numbers as focal points.
>
> Tone: Product-first. Direct. For crypto-native VCs and accelerator reviewers. Lead with "what users get" not "what we built."
>
> Key visuals:
> 1. Title — logo + "One Wallet. Every Stablecoin. Every Chain. Every Bank Account."
> 2. Side-by-side: today (5 steps, 3 platforms, 2 gas tokens) vs Magnus (1 step, 300ms)
> 3. Wallet mockup: USDT/USDC/VND balances + send-to-chain + send-to-bank buttons
> 4. Three giant numbers: $7.2T volume, $1.8B BVNK acquisition, 5 Vietnam exchanges
> 5. Hub-and-spoke: chains on left, banks on right, Magnus center
> 6. Netting flow: 180 USDT in, 130 out, 0 bridged, $0.05 fee, no gas tokens
> 7. Vietnam corridor: $19B, 6.49% fees, 5 exchanges, US→VN arrow
> 8. 2x2 matrix: chains x fiat rails. Codex (few/no), Arc+Magnus (many/no), BVNK (few/yes), TS (many/yes)
> 9. Three-column: Stellar vs Magnus on gateways, EVM, compliance. "SMTP vs Gmail."
> 10. Revenue stack: 4 streams with % and dollar projections
> 11. Timeline: Alliance DAO → Unchained Summit → Devnet → Bridges → Full Stack. Red month-3 kill criterion.
> 12. Dark close: "125 chains. 195 countries. One wallet. 300ms. Zero gas fees."
