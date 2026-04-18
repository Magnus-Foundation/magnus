# Alliance DAO ALL18 Application Draft

Early Admission Deadline: May 27, 2026
Program Start: September 7, 2026 (NYC)

---

## What are you building?

Magnus: one wallet for every stablecoin, every chain, every bank account. No destination gas fees.

Your stablecoins live on Magnus. From one balance you can send to any blockchain (Ethereum, Solana, Tron) or any bank account (VietQR, M-Pesa, UPI, PIX) without holding gas tokens on destination chains. One address to receive from anywhere. 300ms finality.

The core innovation is the Fiat Gateway Protocol, a chain-native standard that makes local payment rail integrations first-class protocol operations, not application-layer hacks. Every gateway integration deepens the moat. Combined with a multilateral netting engine (targeting 90%+ efficiency), most cross-chain transfers settle instantly without touching a bridge, and the protocol absorbs destination gas fees from settlement revenue.

Built on Commonware Simplex consensus (200ms blocks, deterministic finality). Native Payment Engine with PathPayment (multi-hop cross-asset settlement), built-in FX order book, protocol-level compliance pipeline, and Commonware threshold BLS bridges (same security as the chain itself, no third-party bridge trust).

## What problem does this solve?

To move stablecoins today, you need: the right token on the right chain, native gas tokens on every chain you touch (ETH, SOL, TRX), a bridge that might get hacked ($4.3B lost), and a separate fiat off-ramp if you want actual money in a bank account. USDT is on 107 chains. USDC on 125. 214 distinct stablecoin assets. The BIS proved this fragmentation is an equilibrium, not a magnusrary bug.

The new stablechains (Arc, Magnus, Codex) solve chain-to-chain for institutional USD users. Nobody gives the 66% of stablecoin holders in emerging markets one wallet that routes to every chain AND every bank account. A worker in Ho Chi Minh City receiving USDT from the US still needs 3-4 manual steps across platforms to get VND in their bank. On Magnus: one transaction, 300ms, no gas tokens needed.

## Why now?

1. Stablecoin monthly volume hit $7.2T (Feb 2026), overtaking ACH for the first time
2. Mastercard acquired BVNK for $1.8B, proving the market pays for stablecoin orchestration
3. "Stablechains" attracted $548M in funding in 2025 as a recognized category
4. Vietnam launched its crypto regulatory sandbox (Resolution 05/2025) with 5 licensed exchanges (Decision 96/QD-BTC, Jan 2026). These exchanges need cross-border settlement rails that don't exist yet
5. $19B/year Vietnam remittance corridor with 6.49% average fees
6. Arc (Circle) and Magnus (Stripe) launched but neither connects to local fiat rails in emerging markets

## How is this different from Stellar?

Stellar is the closest comparison. 10 years, $55B volume, 69 anchors, PathPayment, hub-and-spoke. Three things Magnus does that Stellar cannot:

1. Gateways are protocol-native operations, not application-layer standards (SEP-6/24/31). The chain enforces gateway registration, deposits, withdrawals, and settlements on-chain.
2. Full EVM composability. Stellar has Soroban. The $200B+ DeFi ecosystem runs on EVM. Every registered stablecoin on Magnus is automatically an ERC-20.
3. Protocol-level compliance and netting. Stellar has neither. These are the features regulated institutions need.

Stellar proved the model works. We're building the next-generation version with compliance, EVM, and netting baked into the protocol.

## How is this different from Arc / Magnus / Codex?

Arc (Circle): USDC-only, institutional, no fiat rail connectivity, no USDT support.
Magnus (Stripe): USD-centric, no emerging market focus, no fiat rails.
Codex (Dragonfly): OP Stack L2, enterprise payments, no fiat gateway protocol.

None of them connect to local payment rails as a protocol. That's the gap.

## What's your unfair advantage?

The fiat gateway protocol creates a compounding network effect. Each fiat rail integration (VietQR, M-Pesa, UPI, PIX) makes the network more valuable for every participant. This is the same moat that made SWIFT unkillable for 50 years, applied to stablecoins.

Vietnam is the wedge. 5 newly licensed exchanges need cross-border settlement. $19B remittance corridor. SBV evaluating stablecoin frameworks. No Western chain builder is targeting this market.

## What traction do you have?

Pre-launch. Building devnet. Key milestones:
- Working Commonware Simplex consensus node (forked from Kora reference client)
- Payment Engine with Transfer, PathPayment, and simulated Gateway operations (in development, demo targeting Unchained Summit Da Nang May 28-29)
- Compliance Precompile Specification (draft complete)
- Gateway Protocol Specification (in progress)
- Existing research: 100+ papers analyzed, full competitive landscape mapped (Arc, Magnus, Codex, Stellar, BVNK)

## Revenue model

1. Settlement fees: basis points on settlement volume (0.01-0.05% per transaction)
2. Gas fees: multi-currency gas (pay in any registered stablecoin)
3. Gateway licensing: annual fee for gateway registration on-chain
4. Premium netting: priority settlement for institutional participants

Target: $100K+ monthly settlement volume by end of 2026 with one Vietnamese exchange partner.

## Team

[FILL IN: Your background, relevant experience, why you're the person to build this]

Key points to hit:
- Technical depth (built the Commonware Simplex client fork, deep Rust/EVM experience)
- Market knowledge (researched 100+ papers on stablecoin payments, studied every competing chain)
- Vietnam connection (attending Unchained Summit Da Nang, network in Vietnamese crypto ecosystem)

## What would you do with the Alliance investment?

$500K allocation:
- 60% engineering (hire 1 senior Rust engineer for 6 months, or extend solo runway to 12 months)
- 20% BD and partnerships (travel to Vietnam, Singapore, Philippines for gateway partner onboarding)
- 10% legal/compliance (regulatory counsel for Vietnam sandbox application)
- 10% infrastructure (validator hosting, security audits)

## What's your ask from Alliance?

1. Introductions to stablecoin ecosystem (Circle, Tether, USDC issuers for native token deployment)
2. Introductions to payment operators in SE Asia (the first gateway partners)
3. Technical mentorship from founders who've built L1s or payment infrastructure
4. Investor introductions for seed round after demo day

## Demo / video

[LINK TO: Unchained Summit demo video or screen recording of the Magnus demo showing USDT -> VND via PathPayment + Gateway in 300ms]

---

## Notes for filling out

- Alliance form takes ~20 minutes, keep answers concise
- They care more about the founder than the idea. Lead with WHY you specifically are building this.
- "Teach them something new" — the BIS fragmentation equilibrium finding and the fiat gateway protocol concept are both novel angles they likely haven't seen
- Include the competitive table showing the Stellar gap
- Mention Unchained Summit Da Nang (May 28-29) as immediate next step
- If asked about team size: solo founder, leveraging AI-assisted development (10-100x compression)
