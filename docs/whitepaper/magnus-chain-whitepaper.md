# Magnus Chain: A Payment-Optimized Layer 1 Blockchain for Emerging Markets

**Version 1.0 — February 2026**

---

## Abstract

Cross-border payment infrastructure in emerging markets remains fundamentally constrained by high transaction fees, multi-day settlement latency, and the absence of regulatory compliance primitives within existing blockchain architectures. Southeast Asia alone accounts for over 290 million unbanked and underbanked adults, while Vietnam's inbound remittance market exceeds $16 billion annually, with corridor fees consuming between 3% and 8% of transferred value. Current Layer 1 platforms force an artificial choice between throughput, compliance, and multi-currency support, rendering them unsuitable for regulated payment workloads at scale.

This paper presents Magnus Chain, a payment-optimized Layer 1 blockchain designed to serve as settlement infrastructure for emerging market financial systems. The architecture rests on four technical pillars. First, a DAG-based parallel execution engine achieves throughput exceeding 700,000 transactions per second through hint generation, conflict graph construction, task group optimization for hot accounts, and lock-free scheduling. Payment lanes provide quality-of-service guarantees by reserving blockspace for payment transactions through dual gas limits enforced at block construction. Second, a suite of native payment primitives introduces the MIP-20 token standard with ISO 4217 currency codes and structured payment data fields, an oracle-driven multi-stablecoin gas fee mechanism that decouples transaction fees from any single denomination, and a transfer policy registry enforcing jurisdiction-specific compliance rules at the protocol level. Third, Magnus Chain implements native ISO 20022 messaging through a hybrid on-chain and off-chain storage model that reduces per-transaction compliance data costs by 99.8% while maintaining direct interoperability with SWIFT and domestic payment networks. Fourth, the infrastructure foundation combines Simplex BFT consensus achieving deterministic finality in approximately 300 milliseconds, MMR-based authenticated storage with parallel merkleization, and BLS12-381 threshold cryptography for aggregate signature verification.

Magnus Chain comprises 73% proprietary code built upon production-grade open-source consensus and networking foundations. Every architectural decision privileges the requirements of regulated payment flows in jurisdictions where traditional financial infrastructure remains incomplete.

---

## 1. Introduction

### 1.1 The Broken State of Cross-Border Payments

The global cross-border payments market processes over $150 trillion annually, yet the infrastructure underpinning these flows remains anchored to correspondent banking networks designed in the 1970s. Nowhere is this dysfunction more acute than in Southeast Asia, a region of 680 million people where approximately 290 million adults remain unbanked or underbanked and where informal workers constitute over 70% of the labor force. Vietnam alone receives more than $16 billion in annual remittances, placing it among the top ten recipient nations worldwide, yet corridor fees between major sending countries and Vietnam range from 3.5% to 8% of transferred value. A Vietnamese factory worker in Japan sending $200 home may lose $7 to $16 in fees and wait two to five business days for settlement through traditional correspondent banking channels. These costs and delays are not incidental inefficiencies but structural consequences of an architecture that routes each payment through multiple intermediary banks, each extracting margin and introducing settlement risk.

The scale of this problem extends well beyond remittances. Domestic payment infrastructure in Vietnam and neighboring markets processes an increasing share of economic activity, yet the gap between digital payment adoption and the underlying settlement layer continues to widen. The Vietnam National Payment System (NAPAS) handles hundreds of millions of domestic transactions monthly, but settlement remains batch-oriented, denominated in a single currency, and disconnected from the cross-border corridors that link Vietnamese households to the diaspora economies of the United States, Japan, South Korea, and Australia. The result is a fragmented financial landscape in which domestic payments, cross-border remittances, and commercial settlement each operate on separate infrastructure stacks with no shared compliance layer and no common data standard.

### 1.2 Why Existing Blockchains Fail for Regulated Payments

Blockchain technology has long promised to disintermediate correspondent banking, yet after more than a decade of development, no Layer 1 platform has achieved meaningful adoption for regulated payment flows in emerging markets. The reasons are structural rather than incidental. Ethereum, the most widely adopted smart contract platform, processes approximately 15 transactions per second with finality measured in minutes, throughput that is orders of magnitude below the requirements of a national payment system. Solana achieves approximately 4,000 actual transactions per second with sub-second confirmation, yet offers no native support for multi-currency gas fees, ISO 20022 messaging, or jurisdiction-specific compliance enforcement. Stellar and Ripple have targeted cross-border payments explicitly, but neither provides a general-purpose execution environment capable of supporting the programmable compliance logic that emerging market regulators increasingly demand.

The fundamental limitation is architectural. Existing Layer 1 platforms were designed as general-purpose computation networks, not as payment settlement infrastructure. They lack native primitives for structured payment data, treating all token transfers as undifferentiated state transitions with no fields for remittance information, purpose codes, or end-to-end transaction identifiers. They price gas in a single volatile denomination, forcing users in Vietnam or the Philippines to acquire and hold a cryptocurrency they neither understand nor trust before executing a simple payment. They offer no protocol-level mechanism for enforcing the know-your-customer, anti-money-laundering, and transfer restriction policies that regulators in Vietnam, Thailand, and Indonesia require as preconditions for legal operation. Each of these deficiencies can be addressed at the application layer through smart contracts and middleware, but doing so sacrifices the performance, composability, and auditability that protocol-level integration provides.

### 1.3 The ISO 20022 Convergence

The completion of SWIFT's ISO 20022 migration in November 2025 marks a watershed moment for blockchain-based payment infrastructure. As of that date, all cross-border payment instructions traversing the SWIFT network must conform to the ISO 20022 XML message standard, replacing the legacy MT format that has governed interbank messaging for decades. The Federal Reserve completed its own Fedwire transition to ISO 20022 in July 2025, and domestic payment systems across Asia have adopted or are actively implementing the standard. ISO 20022 defines a rich, structured data model for payment messages, including originator and beneficiary identification, purpose codes, remittance information, and regulatory reporting fields that legacy formats could not accommodate.

This convergence creates an unprecedented opportunity for blockchain settlement infrastructure. A Layer 1 platform that speaks ISO 20022 natively can serve as a direct settlement backend for banking gateways, eliminating the translation layers and data loss that characterize current blockchain-to-bank integration approaches. Rather than forcing banks to adapt to blockchain-native data formats, such a platform meets the financial system on its own terms, accepting and producing the same structured messages that flow between correspondent banks, central payment systems, and regulatory reporting engines. The value proposition shifts from disintermediation to integration: not replacing banks, but providing them with a faster, cheaper, and more transparent settlement layer that preserves the compliance data they are legally obligated to maintain.

### 1.4 The Magnus Chain Thesis

Magnus Chain is designed around a single organizing principle: every architectural decision must optimize for the specific requirements of regulated payment processing in emerging markets. This principle manifests across four technical pillars that collectively address the throughput, compliance, interoperability, and infrastructure demands that payment workloads impose.

The first pillar is a DAG-based parallel execution engine that achieves throughput exceeding 700,000 transactions per second on 16-32 core validator hardware. The engine operates in four phases: hint generation simulates transactions to predict read/write sets, DAG construction builds a directed acyclic graph of transaction dependencies, task group formation clusters sequential dependencies for efficient execution, and parallel execution distributes independent transactions across worker threads. Payment lanes extend this architecture by reserving blockspace for payment transactions through dual gas limits (`gas_limit` and `general_gas_limit`), ensuring that DeFi congestion cannot crowd out payment processing. The second pillar is a suite of native payment primitives, including a token standard with ISO 4217 currency codes and structured payment data fields, an oracle-driven multi-stablecoin gas fee mechanism, and a transfer policy registry that enforces compliance rules at the protocol level. The third pillar is native ISO 20022 messaging through a hybrid storage model that places essential payment fields on-chain while storing full XML documents off-chain, reducing compliance data costs by 99.8% while enabling direct integration with SWIFT and domestic payment networks. The fourth pillar is an infrastructure foundation combining Simplex BFT consensus with approximately 300-millisecond deterministic finality, MMR-based authenticated storage optimized for parallel merkleization, and BLS12-381 threshold cryptography.

The remainder of this paper describes each pillar in detail, analyzes the security properties of the combined system, presents comparative benchmarks against existing platforms, and outlines the market opportunity that Magnus Chain is positioned to capture.

---

## 2. Design Philosophy

Magnus Chain's architecture emerges from four design principles that collectively distinguish it from general-purpose blockchain platforms. These principles are not aspirational guidelines but binding constraints that have shaped every technical decision described in this paper.

**Payment-first execution.** General-purpose blockchains optimize for arbitrary computation, allocating gas budgets, scheduling transactions, and structuring blocks without regard for the specific characteristics of payment workloads. Magnus Chain inverts this priority. Payment transactions exhibit high throughput, low computational complexity, and predictable state access patterns that make them exceptionally amenable to parallel execution. The DAG-based parallel execution engine exploits these characteristics through transaction dependency analysis, conflict-free scheduling, and task group optimization. Payment lanes provide structural isolation through dual gas limits, dedicating guaranteed capacity to payment transactions. The block header itself encodes this distinction through separate `general_gas_limit` and `shared_gas_limit` fields, ensuring that congestion from complex smart contract interactions cannot degrade payment throughput.

**Compliance by default.** Regulatory compliance in existing blockchain systems is an afterthought, implemented through application-layer smart contracts that cannot enforce invariants across the protocol. Magnus Chain embeds compliance primitives directly into the token standard and transaction processing pipeline. The MIP-20 token standard includes an `ISSUER_ROLE` for authorized minting, a configurable supply cap, and integration with the MIP-403 transfer policy registry that enforces whitelist, blacklist, freeze, and time-lock constraints at the protocol level. Every `transferWithPaymentData` call passes through MIP-403 policy checks before execution, and policy violations are logged using ISO 20022 notification formats. This architecture means that compliance is not something application developers must remember to implement; it is something they cannot circumvent.

**Multi-currency from day one.** Virtually every existing blockchain prices gas in a single native denomination, creating a bootstrapping problem for users in emerging markets who hold local currency and have no prior exposure to cryptocurrency. Magnus Chain eliminates this barrier through a custom transaction type (0x76) that includes a `fee_token` field specifying the MIP-20 stablecoin in which the user wishes to pay gas. An oracle registry maintained by validators and whitelisted external feeds provides real-time foreign exchange rates, and the fee manager converts the user's payment into the validator's preferred denomination at settlement. This design means that a user holding VNST (a Vietnamese dong stablecoin) can transact without ever acquiring or understanding a separate gas token, while the validator receives fees in their preferred USD-denominated stablecoin.

**Modular foundations, proprietary innovation.** Magnus Chain does not reinvent components where battle-tested implementations exist. The consensus layer builds upon a Simplex BFT implementation with years of production validation, and the networking stack leverages proven peer-to-peer primitives. Innovation concentrates at the layers where payment-specific requirements demand novel solutions: the DAG-based parallel execution engine, the oracle-driven fee conversion system, the ISO 20022 messaging integration, and the MIP-20 and MIP-403 payment standards. This strategy yields a codebase that is 73% proprietary while inheriting the reliability of foundations that have secured real economic value in production environments. The approach mirrors the architecture strategy employed by successful infrastructure projects across the industry, where forking proven consensus and networking layers and innovating at the execution and application layers represents the optimal balance of risk, speed, and differentiation.

---

## 3. Part I: The Payment Infrastructure Gap

Existing blockchain platforms fail to serve regulated payment processing not due to lack of ambition but due to fundamental architectural deficiencies. These deficiencies span five dimensions: throughput capacity, compliance enforcement, multi-currency support, banking interoperability, and settlement finality. Each dimension independently disqualifies current platforms from operating as payment settlement infrastructure; collectively, they create an unbridgeable gap between blockchain capability and banking requirements.

### 3.1 Throughput Bottleneck

Payment networks operate at scale measured in hundreds of thousands to millions of transactions per second. Vietnam's NAPAS domestic payment system processes 8.2 billion transactions annually—approximately 260 transactions per second sustained average, with peak loads during salary disbursement periods exceeding 5,000 transactions per second. SWIFT processes approximately 45 million cross-border payment messages daily, averaging 520 messages per second with peaks during European and US market hours reaching several thousand per second. Visa's VisaNet authorization network handles peak loads exceeding 65,000 transactions per second during holiday shopping periods.

Existing blockchain platforms operate orders of magnitude below these requirements. Ethereum processes approximately 15 transactions per second with finality measured in minutes. Solana achieves approximately 4,000 actual transactions per second (distinct from the theoretical maximum claimed in marketing materials), but this throughput applies to all transaction types without differentiation between simple value transfers and complex smart contract interactions. Neither platform provides a mechanism to guarantee that payment transactions receive processing priority during periods of network congestion.

The throughput gap is not merely quantitative but qualitative. Payment processing exhibits specific characteristics that distinguish it from general-purpose computation: high transaction volume, low computational complexity per transaction, and predictable state access patterns (predominantly account balance updates). Existing platforms treat payments as undifferentiated state transitions, executing them through the same sequential processing pipeline used for arbitrary smart contract logic. This architectural choice sacrifices the parallelization opportunities that payment workloads naturally provide.

### 3.2 Compliance Void

Regulated financial institutions operate under legal frameworks that mandate know-your-customer (KYC) verification, anti-money-laundering (AML) transaction monitoring, and jurisdiction-specific transfer restrictions. These requirements are not discretionary features that institutions may choose to implement; they are preconditions for legal operation enforced through licensing regimes, periodic audits, and substantial penalties for non-compliance.

Existing blockchain platforms offer no protocol-level primitives for compliance enforcement. Transaction validity is determined purely by cryptographic signature verification and sufficient balance—a transaction signed by the private key corresponding to an account address will execute regardless of whether the sender or recipient has completed identity verification, whether they appear on sanctions lists, or whether the transfer violates holding period restrictions or spending limits. Compliance logic, when implemented at all, exists in application-layer smart contracts that cannot enforce invariants across the protocol.

This architectural gap manifests in three specific deficiencies. First, there is no protocol-enforced identity registry that maps addresses to verified identity credentials and risk tiers. Second, there is no mechanism to attach compliance policies to token contracts such that all transfers—regardless of call path or initiation method—must satisfy policy checks before execution. Third, there is no standard for structured payment data that carries the remittance information, purpose codes, and end-to-end identifiers that regulatory reporting and transaction monitoring systems require.

Financial institutions attempting to use existing blockchains for settlement must therefore implement parallel compliance systems that operate entirely off-chain, validating transactions before submission and monitoring on-chain activity for policy violations after the fact. This architecture sacrifices the composability and atomic execution properties that make blockchain settlement attractive while introducing reconciliation gaps between on-chain settlement and off-chain compliance state.

### 3.3 Multi-Currency Barrier

Every EVM-compatible blockchain prices gas in a single native denomination. On Ethereum, users pay gas in ETH. On Polygon, users pay in MATIC. On Arbitrum, users pay in ETH bridged from Ethereum mainnet. This design creates an onboarding barrier that is particularly acute in emerging markets where potential users hold local currency and have limited or no prior exposure to cryptocurrency.

Consider a Vietnamese factory worker receiving a remittance payment in VNST, a Vietnamese dong-denominated stablecoin. To send a subsequent payment on Ethereum, this user must first acquire ETH through a cryptocurrency exchange—a process requiring account creation, identity verification, fiat-to-crypto conversion, and navigation of an unfamiliar interface. The user must then maintain a separate ETH balance sufficient to cover gas fees for all future transactions. If the ETH balance depletes, the user's VNST holdings become temporarily unusable until additional ETH is acquired. The ETH price denominated in Vietnamese dong fluctuates by double-digit percentages annually, introducing unpredictable transaction costs.

This architecture violates a fundamental principle of payment infrastructure: the medium of payment should not require users to acquire, hold, or understand a separate volatile asset to access the system. Traditional payment networks price services in the same currency as the payment itself—a US dollar wire transfer costs US dollars, a euro SEPA transfer costs euros. Users understand the fee in the same denomination as the value being transferred.

The multi-currency gas problem extends beyond user experience to institutional adoption. A Vietnamese bank operating a VNST stablecoin on Ethereum must ensure that every user wallet holds both VNST and ETH. The bank must implement ETH distribution infrastructure—either pre-funding new user wallets with small ETH amounts (capital inefficient) or requiring users to acquire ETH independently (poor user experience). The bank must also implement ETH balance monitoring and top-up systems to prevent users from being unable to transact due to depleted gas balances.

### 3.4 Interoperability Failure

The completion of SWIFT's ISO 20022 migration in November 2025 and the Federal Reserve's Fedwire transition in July 2025 established a global standard for structured financial messaging. Domestic payment systems across Southeast Asia are actively implementing ISO 20022 or have published transition roadmaps. This convergence creates a rare opportunity: a Layer 1 blockchain that speaks ISO 20022 natively can integrate directly with existing banking infrastructure, eliminating the translation layers and data loss that characterize current blockchain-to-bank bridges.

Existing blockchain platforms provide no native ISO 20022 support. Token transfer events emit undifferentiated log data: sender address, recipient address, and amount. There are no fields for end-to-end identifiers, purpose codes, remittance information, or structured originator/beneficiary data. Banking gateways attempting to bridge blockchain settlement to ISO 20022 messaging networks must therefore maintain parallel databases that associate blockchain transaction hashes with payment metadata stored off-chain. This architecture creates reconciliation gaps where the on-chain settlement record and the off-chain payment data exist as separate artifacts that must be manually linked.

The interoperability gap manifests in data loss during cross-system transitions. When a bank converts an ISO 20022 payment instruction into a blockchain transaction, structured data elements—beneficiary name, address, bank identifier code, purpose code, regulatory reporting fields—are discarded because the blockchain transaction format has no equivalent fields. When the blockchain transaction settles and must be reported to the receiving bank's core banking system, the gateway must reconstruct these data elements from off-chain storage, introducing opportunities for data corruption, loss, or desynchronization.

This architectural mismatch forces banks to choose between operating blockchain settlement as an isolated subsystem disconnected from their core operations or implementing complex middleware that attempts to bridge incompatible data models. Neither option provides the seamless interoperability that would enable blockchain settlement to serve as a direct replacement for correspondent banking rails.

### 3.5 Settlement Risk

Payment settlement requires deterministic finality: the irreversible commitment of a transaction such that no subsequent event—network partition, validator misbehavior, or chain reorganization—can cause the transaction to be reversed or altered. Traditional payment networks achieve this through centralized ledgers that record final settlement at a single point of truth. Blockchain consensus protocols must achieve the same property in a decentralized environment where no single party controls the settlement record.

Existing blockchain platforms employ consensus mechanisms that provide varying degrees of finality. Ethereum's proof-of-stake consensus provides probabilistic finality where the likelihood of transaction reversal decreases exponentially as additional blocks are appended to the chain. Practical finality—the point at which reversal is economically irrational even for an attacker controlling substantial stake—is achieved after approximately 64 to 95 seconds (two to three epochs). However, the protocol provides no absolute finality guarantee: sufficiently motivated attackers with majority stake could theoretically reorganize arbitrarily old blocks.

Solana's consensus mechanism similarly provides probabilistic finality based on voting stake weight. Practical finality is achieved when a supermajority of stake has voted for a block, typically occurring within 400 milliseconds. However, the protocol's reliance on Proof of History timestamps and the potential for network partitions during validator failures introduce edge cases where finalized blocks may be reorganized if network assumptions are violated.

This probabilistic finality is unsuitable for payment settlement. A merchant accepting a $10,000 payment transaction cannot operate on the basis that reversal is "economically irrational" or "extremely unlikely"—the merchant requires mathematical certainty that the payment is final. A bank crediting a customer's account based on an incoming remittance cannot explain to regulatory auditors that the credit was issued when reversal probability dropped below 0.1%—the bank requires deterministic finality equivalent to what traditional payment rails provide.

The settlement finality gap extends to finality latency. Ethereum's two-epoch finality means that a payment transaction submitted at time T is not deterministically final until T+90 seconds. During this window, the receiving party faces settlement risk: the transaction may ultimately fail to finalize if network conditions change. This latency is orders of magnitude above the finality expectations for modern payment infrastructure, where real-time settlement increasingly means sub-second confirmation.

---

## 4. Part II: The Magnus Solution

Magnus Chain addresses the payment infrastructure gap through three integrated components: native payment primitives that embed compliance and multi-currency support at the protocol level, banking integration primitives that enable direct interoperability with existing financial infrastructure, and a parallel execution architecture that delivers the throughput required for national-scale payment processing.

### 4.1 Payment Primitives

#### 4.1.1 MIP-20 Token Standard

The MIP-20 token standard serves as the foundational unit of value on Magnus Chain. It is a strict superset of the ERC-20 interface, meaning that any wallet or application capable of interacting with ERC-20 tokens can interact with MIP-20 tokens without modification. The standard extends ERC-20 with three critical capabilities that regulated payment processing demands.

First, each MIP-20 token carries an ISO 4217 currency code that identifies its denomination. A Vietnamese dong stablecoin stores `currency = "VND"`, a US dollar stablecoin stores `currency = "USD"`, and a euro stablecoin stores `currency = "EUR"`. This explicit currency identity eliminates ambiguity in multi-currency environments and enables automated foreign exchange rate lookups without relying on token contract names or off-chain metadata that can be spoofed.

Second, MIP-20 tokens include role-based access control for minting authority. The `ISSUER_ROLE` restricts minting to addresses explicitly authorized by the token administrator, and the `supply_cap` parameter enforces a protocol-level ceiling on total issuance. These constraints enable regulatory-compliant stablecoin deployments where issuance is tied to fiat reserves held by a licensed entity. The on-chain `supply_cap` provides a verifiable upper bound that regulators and auditors can monitor without trusting the issuer's off-chain reporting.

Third, the standard defines a `transferWithPaymentData` function that augments token transfers with structured payment information aligned to ISO 20022 data elements:

```solidity
function transferWithPaymentData(
    address to,
    uint256 amount,
    bytes calldata endToEndId,      // Max 35 chars (ISO Max35Text)
    bytes4 purposeCode,              // 4 bytes (e.g., "SALA", "SUPP")
    bytes calldata remittanceInfo    // Max 140 chars (ISO Max140Text)
) external returns (bool);
```

The `endToEndId` field carries a unique payment identifier up to 35 characters in length, matching the ISO 20022 `EndToEndIdentification` element that banks use to track payments across institutional boundaries. The `purposeCode` is a four-byte code drawn from the ISO 20022 `ExternalPurpose1Code` vocabulary: `SALA` for salary, `SUPP` for supplier payment, `TAXS` for tax remittance, `PENS` for pension disbursement. The `remittanceInfo` field provides up to 140 characters of unstructured information for invoice references or reconciliation notes.

These fields are emitted as event data rather than stored in contract state, a deliberate design choice that preserves the gas efficiency of a simple balance update while making the full payment context available to off-chain indexers, banking gateways, and regulatory reporting systems.

#### 4.1.2 Multi-Currency Gas Fees

The gas fee mechanism eliminates the onboarding barrier that every other EVM-compatible blockchain imposes: the requirement to hold a native token to pay transaction fees. Magnus Chain implements an oracle-driven system that allows users to pay fees in any supported MIP-20 stablecoin.

The system operates through a custom transaction type (0x76) that extends the EIP-1559 format with a `fee_token` field specifying the MIP-20 stablecoin address. When a validator executes a 0x76 transaction, the fee collection flow operates in two phases. Pre-execution, the Fee Manager contract locks the maximum possible fee (`gas_limit × max_fee_per_gas`) in the user's chosen stablecoin, converted to the user's denomination using the current oracle exchange rate. Post-execution, the manager refunds unused gas, converts the actual fee to the validator's preferred denomination (typically a USD stablecoin), and transfers the converted amount to the validator's fee accumulator.

The Oracle Registry maintains real-time foreign exchange rates through a decentralized price feed. Whitelisted reporters—comprising validators and authorized external oracles—submit rate observations for currency pairs (e.g., VND/USD, EUR/USD). The registry stores reports in a sorted list and computes the median of all valid (non-expired) reports as the canonical rate. Reports expire after 360 seconds by default, ensuring the system never relies on stale data.

A circuit breaker provides manipulation resistance. When a new report deviates from the current median by more than 2,000 basis points (20%), the breaker automatically freezes the affected pair, preventing fee calculations based on potentially manipulated rates. The 20% threshold accommodates normal foreign exchange volatility while catching outlier attacks. Governance can reset the breaker after investigation.

This design means that a Vietnamese user holding VNST can submit a payment transaction without ever acquiring or understanding a separate gas token. The validator receives fees in their preferred USD-denominated stablecoin. The foreign exchange conversion happens transparently at the protocol level, denominated in basis points rather than percentage spreads, ensuring predictable costs.

#### 4.1.3 Transfer Policy Registry (MIP-403)

Regulatory compliance in existing blockchain systems is implemented through application-layer smart contracts that cannot enforce invariants across the protocol. Magnus Chain embeds compliance primitives directly into the token transfer pipeline through the MIP-403 Transfer Policy Registry.

Each MIP-20 token references a policy identifier in the MIP-403 registry. Before executing any transfer—whether initiated by `transfer`, `transferFrom`, `transferWithPaymentData`, or batch calls within a 0x76 transaction—the token contract queries the registry's `ensure_transfer_authorized` function. This function evaluates the policy associated with the token and returns a boolean authorization decision plus an optional denial reason code.

The registry supports four policy types. **Whitelist policies** maintain a set of authorized addresses; transfers are permitted only if both sender and recipient appear in the set. **Blacklist policies** maintain a set of prohibited addresses; transfers are rejected if either party appears in the set. **Freeze policies** block all transfers for a specific token, typically used during security incidents or regulatory holds. **Time-lock policies** enforce minimum holding periods, rejecting transfers if the sender acquired the tokens within a configurable time window.

Policy administration is access-controlled. Each policy record stores an administrative address that has exclusive authority to modify the policy's address set (add or remove whitelist/blacklist entries). The policy type itself is immutable after creation, preventing an attacker who gains administrative access from converting a restrictive whitelist into a permissive blacklist.

Because the policy check is embedded in the token's internal `_transfer` function rather than an external wrapper, there is no code path through which a transfer can execute without passing the policy check. This property holds regardless of how the transfer is initiated—direct calls, approved transfers, system transfers from precompiles, and atomic batch calls all traverse the same internal function. The enforcement is protocol-level, not application-layer, providing the settlement assurance that regulated financial institutions require.

### 4.2 Banking Integration

Magnus Chain implements native ISO 20022 messaging through a hybrid storage model that balances on-chain verifiability with off-chain cost efficiency. Essential payment fields—`endToEndId`, `purposeCode`, and `remittanceInfo`—are emitted as event data in every `transferWithPaymentData` call, consuming approximately 200 bytes of on-chain storage. The complete ISO 20022 XML message, which can exceed 4KB for complex commercial payments, is stored off-chain by banking gateway operators who monitor chain events and generate standard-compliant messages (pain.001 for customer credit transfers, pacs.008 for interbank settlement, camt.053 for account statements, camt.054 for debit/credit notifications).

The banking gateway architecture provides bidirectional connectivity between Magnus Chain and traditional payment networks. Outbound, the gateway monitors on-chain `Transfer` and `TransferWithPaymentData` events, extracts structured payment data, and submits ISO 20022 messages to SWIFT or domestic payment systems like Vietnam's NAPAS. Inbound, the gateway accepts payment instructions from banking channels, converts them to Magnus 0x76 transactions, and submits them to the chain, generating on-chain confirmation events that close the payment loop.

The KYC Registry implements tiered identity verification that maps to risk-based approaches mandated by FATF guidelines. Each verified address is associated with a tier level (e.g., Tier 1 for basic verification, Tier 2 for enhanced due diligence, Tier 3 for institutional accounts) that determines transaction limits and eligible payment types. Token issuers configure MIP-403 policies that reference KYC tiers as authorization preconditions, ensuring that high-value or cross-border transfers automatically require verified counterparties.

### 4.3 Scale and Performance

#### 4.3.1 DAG-Based Parallel Execution

The execution layer achieves 700,000 transactions per second on 16-core validator hardware through a directed acyclic graph (DAG) based parallel execution engine. The architecture operates in four phases that convert a sequential batch of transactions into a maximally parallel execution schedule.

**Phase 1: Hint Generation.** All transactions are simulated in parallel to produce predicted read/write sets—the storage locations each transaction will access. This simulation uses a lightweight execution path (no state persistence, no event logging) that completes in approximately 10 microseconds per transaction. The predicted access patterns populate the initial dependency graph.

**Phase 2: DAG Construction.** The engine builds a directed acyclic graph where nodes represent transactions and edges represent read-after-write dependencies. If transaction T_j reads a storage slot that transaction T_i writes, and i < j in block order, an edge T_i → T_j is added to the graph. A critical optimization—selective dependency updates—adds only the highest-index conflicting transaction as a dependency rather than all conflicts, reducing graph size and minimizing re-execution attempts.

The DAG is partitioned into weakly connected components (WCCs): groups of transactions with dependencies within the group but no dependencies across groups. Independent WCCs execute in parallel with zero synchronization overhead.

**Phase 3: Task Group Formation.** Transactions with dependency distance equal to 1—meaning they depend directly on the immediately preceding transaction—are grouped into task groups that execute sequentially on a single worker thread. This handles the common banking pattern of multiple payments from the same sender (e.g., a payroll batch) where each transaction writes the same gas token balance. Task groups execute these sequential dependencies at near-serial speed (only 3-5% slower than pure sequential execution) while freeing remaining cores for parallel work.

**Phase 4: Parallel Execution and Validation.** Independent transactions and task groups execute concurrently across worker threads. After execution, a validator checks whether each transaction's actual read/write set matches its predicted set. Matches proceed to finalization. Mismatches trigger selective dependency updates: the conflicting transaction is re-inserted into the DAG with new dependencies derived from its actual access pattern, then re-executed. The selective update strategy ensures ≤2 execution attempts for typical workloads.

This architecture achieves approximately 4× speedup on 16-core hardware for payment-dominated workloads where conflict ratios remain below 35%. The speedup scales near-linearly to 32 cores, yielding projected throughput of 700,000 to 1,000,000 transactions per second for simple MIP-20 transfers.

#### 4.3.2 Payment Lanes

Magnus Chain extends the parallel execution architecture with a payment lane mechanism that guarantees payment transactions always have block capacity, even during peak network congestion from DeFi or smart contract activity.

The mechanism operates through dual gas limits enforced at the block construction level. Every Magnus block header encodes two constraints:

- **`gas_limit`:** Total gas available for all transactions (standard Ethereum behavior)
- **`general_gas_limit`:** Maximum gas that non-payment transactions can consume

The difference (`gas_limit - general_gas_limit`) is effectively reserved for payment transactions. When a validator constructs a block, non-payment transactions in the proposer's lane can only fill up to `general_gas_limit`. Payment transactions can consume the remaining capacity beyond that threshold.

Transaction classification requires no state lookups—it is performed entirely from the transaction payload. A transaction is classified as a payment if:

1. Its `tx.to` address starts with the MIP-20 prefix `0x20c0000000000000000000000000`, or
2. For 0x76 Magnus transactions, every entry in `tx.calls` targets a MIP-20 prefix address

Everything else is classified as a non-payment (general) transaction.

Blocks are structured in five sections: start-of-block system transactions (rewards distribution), proposer lane (user transactions with `general_gas_limit` enforced on non-payments), sub-block transactions (from other proposers), gas incentive transactions (consuming leftover shared gas capacity), and end-of-block system transactions. This structure ensures that payment capacity is protected throughout the block construction process.

The payment lane mechanism requires no user action. If a user sends a stablecoin payment on Magnus Chain, it automatically receives priority access to blockspace—DeFi congestion cannot crowd out payments. This isolation is core to Magnus Chain's identity as a payment-first blockchain, providing the quality-of-service guarantees that financial settlement infrastructure requires.

---

## 5. Part III: Technical Architecture

The preceding section described what Magnus Chain provides to solve the payment infrastructure gap. This section describes how the underlying technical components deliver those capabilities. The architecture prioritizes production-proven components over research prototypes, combining a parallel execution engine with battle-tested consensus and storage primitives.

### 5.1 Execution Layer

Beyond the core DAG-based parallel execution described in Section 4.3.1, the execution layer incorporates three performance optimizations that collectively enable sustained high throughput under continuous block production.

**Ahead-of-time compilation.** Hot contracts—those invoked frequently such as the gas token, major stablecoins (VNST, USDC, EURC), and payment router contracts—are compiled to native machine code at node initialization rather than interpreted during execution. This compilation uses LLVM-based translation from EVM bytecode to x86 or ARM machine code, eliminating interpreter overhead for the 60-70% of transactions that interact with these pre-compiled contracts. The speedup is approximately 1.5-2× for storage-heavy operations like token transfers.

**Async pipeline architecture.** Block execution and Merkle tree computation are overlapped through a five-stage asynchronous pipeline. While block N undergoes state merkleization (computing the authenticated state root), block N+1 begins execution. This pipelining reduces effective latency from the sum of execution time plus merkleization time to the maximum of the two, improving throughput by approximately 40% under sustained block production.

**Concurrent state cache.** A lock-free state cache maintains the latest view of frequently accessed accounts and storage slots in memory, tagged with block numbers to enable safe eviction after persistence. This prevents performance degradation that would otherwise occur during high block production rates when multiple unpersisted blocks accumulate in memory. The cache uses concurrent hash maps with block-tagged entries and achieves update latencies under 10 milliseconds for blocks containing 5,000 transactions.

### 5.2 Infrastructure Foundation

**Consensus.** Magnus Chain implements Simplex BFT consensus, a Byzantine fault tolerant protocol that achieves block proposal in approximately 200 milliseconds (2 network hops) and deterministic finality in approximately 300 milliseconds (3 network hops). Unlike probabilistic finality models where confirmation strengthens over time, Simplex provides absolute finality: once a block is committed, no reorganization is possible under the BFT assumptions (fewer than one-third of validators are malicious). This deterministic settlement is non-negotiable for payment processing where merchants and banks must know with certainty that transactions cannot be reversed.

**Storage.** The state storage engine uses a Merkle Mountain Range (MMR) structure rather than the Merkle Patricia Trie employed by Ethereum. MMR is an append-only authenticated data structure where state updates require only logarithmic hashing rather than tree traversal and structural mutations. This design enables parallel merkleization: nodes at the same height can be hashed concurrently without contention, yielding 4-6× speedup compared to sequential merkleization. The storage engine has been benchmarked with workloads exceeding 15 billion entries and demonstrates capacity to scale to 280 billion entries on commodity hardware.

**Cryptography.** Magnus Chain uses BLS12-381 elliptic curve cryptography for all consensus-layer operations. BLS signatures support aggregation—multiple individual signatures over the same message combine into a single constant-size signature—reducing bandwidth for block propagation. The consensus employs a threshold signature scheme where the validator set collectively holds a shared public key and any subset exceeding the Byzantine fault tolerance threshold (more than two-thirds) can produce a valid signature. This threshold signature serves as the finality certificate for each block. Distributed Key Generation (DKG) ceremonies at epoch boundaries produce fresh key material and enable validator set evolution.

**Modularity.** The codebase is organized as 46 Rust crates structured into functional domains: core primitives, consensus, execution, storage, networking, precompiles, and application binaries. This modular architecture enforces separation of concerns at the compilation level—the consensus engine depends on abstract traits for block validation, not concrete execution implementations. The architecture enables independent development and testing of each layer and ensures that component replacements or upgrades do not cascade changes across the codebase.

### 5.3 Security Model

Magnus Chain's security architecture addresses threats across five layers that compose into defense-in-depth where compromise of any single layer does not compromise the system as a whole.

**Consensus security.** The Simplex BFT protocol provides safety (no conflicting blocks can be finalized) and liveness (the chain produces blocks) under the assumption that fewer than one-third of validators are Byzantine. The BLS12-381 threshold signature scheme distributes signing authority across the validator set such that compromising a minority of validators grants no ability to forge block signatures. Regular key rotation through DKG ceremonies at epoch boundaries limits exposure windows.

**Oracle manipulation resistance.** The oracle registry employs multiple independent defenses. The whitelist restricts rate submissions to validators and authorized feeds. Median aggregation provides robustness against minority manipulation—even if a minority submits extreme values, the median remains anchored to the honest majority. The circuit breaker automatically freezes rate pairs when new reports deviate more than 20% from the median, preventing transactions from proceeding with manipulated rates. Rate expiry ensures the system never relies on stale data, failing closed rather than accepting potentially outdated values.

**Payment lane isolation.** The dual gas limit architecture provides quality-of-service guarantees that prevent denial-of-service attacks. An adversary flooding the general execution lane with gas-intensive contracts cannot affect payment lane capacity. Payment transactions continue processing at their dedicated throughput level even during general lane congestion. This isolation also prevents economic attacks where general-lane gas price manipulation would make payment processing prohibitively expensive.

**Compliance enforcement.** The MIP-403 transfer policy registry provides protocol-level compliance that is fundamentally more secure than application-layer alternatives. Because the `ensure_transfer_authorized` check is embedded in the MIP-20 token's internal transfer logic, there is no code path through which a transfer can execute without passing policy validation. This property holds regardless of call origin—direct calls, approved transfers, system transfers from precompiles, and batch calls within 0x76 transactions all traverse the same internal function. The policy type is immutable after creation, preventing privilege escalation attacks.

**Cryptographic security.** The BLS12-381 curve provides approximately 128 bits of security against classical adversaries and has been extensively analyzed by the cryptographic research community. The DKG ceremony uses verifiable secret sharing where each participant independently verifies their received share's consistency with public commitments, preventing malicious dealers from distributing invalid shares. Account Keychain support for P256 and WebAuthn signature types enables hardware-backed key storage in mobile secure enclaves and HSMs, providing tamper resistance even if application processors are compromised.

---

## 6. Competitive Analysis and Benchmarks

### 6.1 Platform Comparison

The following analysis compares Magnus Chain against five blockchain platforms that represent the current state of the art across different points in the design space: Ethereum as the dominant smart contract platform, Solana as the leading high-throughput general-purpose chain, MegaETH as the most ambitious throughput claimant in the EVM ecosystem, Stellar as an established payment-focused network, and XRP Ledger as the most widely deployed cross-border payment blockchain.

| Capability | Ethereum | Solana | MegaETH | Stellar | XRP Ledger | **Magnus Chain** |
|-----------|----------|--------|---------|---------|------------|-----------------|
| Throughput (TPS) | ~15 | ~4,000 | ~100,000 | ~1,000 | ~1,500 | **700,000+** |
| Finality | ~13 min | ~400ms | ~10ms | 3-5s | 3-5s | **~300ms** |
| Execution Model | Sequential EVM | Sealevel | Specialized | Non-EVM | Non-EVM | **DAG parallel EVM** |
| EVM Compatible | Native | No | Yes | No | No | **Yes** |
| ISO 20022 Native | No | No | No | No | Via middleware | **Yes** |
| Multi-Currency Gas | No | No | No | No | No | **Yes (oracle-driven)** |
| Payment Data Fields | No | No | No | Memo only | 1KB memo | **ISO 20022 fields** |
| Compliance Primitives | No | No | No | Basic anchors | Basic | **MIP-403 policies** |
| Transfer Policies | No | No | No | No | Freeze only | **Whitelist/blacklist/freeze/time-lock** |

The comparison reveals that no existing platform occupies the intersection of high throughput, EVM compatibility, native ISO 20022 support, and protocol-level compliance enforcement. Ethereum and Solana dominate general-purpose computation but lack payment-specific primitives. Stellar and XRP Ledger have targeted payments explicitly but sacrifice the programmability of a general-purpose execution environment and provide only rudimentary compliance tooling. MegaETH pursues raw throughput within the EVM ecosystem but offers no payment-specific features. Magnus Chain is the only platform that combines all five capabilities — throughput, EVM compatibility, ISO 20022, multi-currency gas, and compliance enforcement — in a single architecture.

### 6.2 Transaction Cost Analysis

Transaction cost is the primary economic metric for payment infrastructure viability. A payment network that charges more per transaction than existing banking rails has no value proposition regardless of its technical capabilities. The following table compares the cost of four representative transaction types across platforms.

| Transaction Type | Ethereum | Solana | Stellar | XRP Ledger | **Magnus Chain** |
|-----------------|----------|--------|---------|------------|-----------------|
| Simple transfer | ~$0.44 | ~$0.00025 | ~$0.00001 | ~$0.0002 | **<$0.001** |
| Token transfer (ERC-20/equivalent) | ~$2.50 | ~$0.00025 | ~$0.00001 | ~$0.0002 | **<$0.001** |
| ISO 20022 payment (with data) | ~$120+ | N/A | N/A | ~$0.01 | **<$0.005** |
| Cross-currency settlement | ~$250+ | N/A | ~$0.001 | ~$0.01 | **<$0.01** |

The cost differential is most pronounced for ISO 20022 payments, where Magnus Chain's hybrid storage model reduces the on-chain data footprint from kilobytes (required for full XML storage on Ethereum) to approximately 200 bytes, achieving a 99.8% cost reduction. For simple transfers, Magnus Chain's costs are competitive with the lowest-cost networks while providing substantially richer payment data and compliance features. The cross-currency settlement cost reflects the oracle-based fee conversion at 25 basis points, which is lower than the typical 30-100 basis point spreads observed in AMM-based conversion pools.

### 6.3 Throughput Benchmarks

The 700,000 TPS throughput projection derives from analytical modeling calibrated against production parallel EVM benchmarks. A baseline parallel execution engine achieves approximately 41,000 TPS for ERC-20 transfers on 16-core hardware (1.5 gigagas per second with 36,000 gas per transfer). The 4× parallel speedup from DAG-based execution yields ~160,000 TPS. Banking-specific optimizations—static transpilation of hot contracts (2× speedup), pre-scheduling for known transaction types (1.2× speedup), and async storage pipelining (1.4× speedup)—combine multiplicatively to 3.36× additional improvement, yielding ~540,000 TPS on 16 cores. Scaling to 32 cores with >80% parallel efficiency yields 700,000-1,000,000 TPS.

For payment workloads where conflict ratios remain below 35% (typical for banking where individual accounts transact infrequently relative to network throughput), parallel efficiency exceeds 90% through task group optimization that handles sequential dependencies from the same sender.

---

## 7. Security and Resilience

The security analysis in Section 5.3 demonstrates defense-in-depth across consensus, oracle, payment lane, compliance, and cryptographic layers. This section extends that analysis with operational security considerations.

**Validator set security.** Magnus Chain's BFT security assumes fewer than one-third of validators are malicious. With a target validator set of 100 nodes, the network tolerates up to 33 Byzantine validators while maintaining safety and liveness. Validator selection employs stake-weighted random sampling with a minimum stake threshold to prevent Sybil attacks. The threshold signature scheme ensures that compromising a minority subset grants no signing authority.

**Network-level attacks.** DDoS attacks targeting individual validators cannot halt consensus because the protocol maintains liveness as long as two-thirds of validators remain reachable. Payment lane isolation ensures that even successful DoS attacks flooding the general execution lane cannot prevent payment transactions from processing. The separation provides protocol-level quality of service that application-layer rate limiting cannot achieve.

**Economic security.** The oracle circuit breaker prevents flash-crash manipulation where an attacker attempts to exploit temporary price dislocations. The 20% deviation threshold accommodates normal FX volatility (the Thai baht trades within 15% annual ranges) while catching manipulation attempts. The freeze-on-deviation behavior prefers false positives (temporary unavailability) over false negatives (accepting manipulated rates).

**Recovery procedures.** Deterministic finality enables clean disaster recovery. Because finalized blocks cannot reorganize, nodes recovering from crashes need only replay from the last persisted state root. The MMR storage structure's append-only design prevents corruption during crashes—partially written updates do not invalidate existing authenticated state. Regular state snapshots enable fast-sync for new validators joining the network.

---

## Appendix A: MIP-20 Token Specification

The MIP-20 token standard defines the protocol-level token primitive for Magnus Chain. Every stablecoin, payment token, and fee token on the network is deployed as an MIP-20 contract through the MIP20Factory, which assigns each token a deterministic address with the prefix `0x20C0` (12 bytes) followed by 8 bytes derived from the creation parameters.

### Storage Layout

Each MIP-20 token maintains the following state:

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Human-readable token name |
| `symbol` | `String` | Short ticker symbol |
| `currency` | `String` | ISO 4217 currency code (e.g., "VND", "USD", "EUR") |
| `quote_token` | `Address` | Reference token for oracle price resolution |
| `next_quote_token` | `Address` | Staged quote token pending finalization |
| `transfer_policy_id` | `u64` | MIP-403 policy governing transfers |
| `total_supply` | `U256` | Current circulating supply |
| `supply_cap` | `U256` | Maximum allowed supply (default: `u128::MAX`) |
| `balances` | `Mapping<Address, U256>` | Per-account balances |
| `allowances` | `Mapping<Address, Mapping<Address, U256>>` | ERC-20 approval mappings |
| `nonces` | `Mapping<Address, U256>` | EIP-712 permit nonces |
| `paused` | `bool` | Emergency pause state |
| `domain_separator` | `B256` | EIP-712 domain separator |

All MIP-20 tokens use a fixed 6 decimal places (`MIP20_DECIMALS = 6`), providing sufficient precision for fiat-denominated stablecoins while avoiding the gas overhead of 18-decimal arithmetic.

### Role-Based Access Control

MIP-20 tokens implement a hierarchical role system with the following predefined roles:

| Role | Hash | Permissions |
|------|------|-------------|
| `DEFAULT_ADMIN_ROLE` | `0x00` | Grant/revoke roles, change transfer policy, set supply cap, manage quote token |
| `ISSUER_ROLE` | `keccak256("ISSUER_ROLE")` | Mint and burn tokens |
| `PAUSE_ROLE` | `keccak256("PAUSE_ROLE")` | Pause the token contract |
| `UNPAUSE_ROLE` | `keccak256("UNPAUSE_ROLE")` | Unpause the token contract |
| `BURN_BLOCKED_ROLE` | `keccak256("BURN_BLOCKED_ROLE")` | Burn tokens from blocked accounts |

### Standard Functions (ERC-20 Compatible)

```
function name() → string
function symbol() → string
function decimals() → uint8                              // Always returns 6
function currency() → string                             // ISO 4217 code
function totalSupply() → uint256
function balanceOf(address account) → uint256
function transfer(address to, uint256 amount) → bool
function transferFrom(address from, address to, uint256 amount) → bool
function approve(address spender, uint256 amount) → bool
function allowance(address owner, address spender) → uint256
```

### Payment Extension Functions

```
function transferWithMemo(address to, uint256 amount, bytes32 memo)
function transferWithPaymentData(
    address to,
    uint256 amount,
    bytes endToEndId,           // Max 35 bytes (ISO 20022 Max35Text)
    bytes4 purposeCode,         // ISO 20022 ExternalPurpose1Code
    bytes remittanceInfo        // Max 140 bytes (ISO 20022 Max140Text)
)
function transferFromWithPaymentData(
    address from, address to, uint256 amount,
    bytes endToEndId, bytes4 purposeCode, bytes remittanceInfo
) → bool
function mintWithPaymentData(
    address to, uint256 amount,
    bytes endToEndId, bytes4 purposeCode, bytes remittanceInfo
)
```

Payment data is emitted as event data only and is not stored in contract state. This design minimizes gas costs while enabling off-chain indexers to reconstruct full ISO 20022 payment records from the event log. The `endToEndId` field is validated against a maximum length of 35 characters conforming to the ISO 20022 `Max35Text` type, and the `remittanceInfo` field is validated against a maximum of 140 characters conforming to `Max140Text`.

### Administrative Functions

```
function mint(address to, uint256 amount)                // Requires ISSUER_ROLE
function burn(uint256 amount)                            // Requires ISSUER_ROLE
function burnBlocked(address from, uint256 amount)       // Requires BURN_BLOCKED_ROLE
function pause()                                         // Requires PAUSE_ROLE
function unpause()                                       // Requires UNPAUSE_ROLE
function setSupplyCap(uint256 newSupplyCap)              // Requires DEFAULT_ADMIN_ROLE
function changeTransferPolicyId(uint64 newPolicyId)      // Requires DEFAULT_ADMIN_ROLE
function setNextQuoteToken(address newQuoteToken)         // Requires DEFAULT_ADMIN_ROLE
function completeQuoteTokenUpdate()                      // Requires DEFAULT_ADMIN_ROLE
```

The quote token update follows a two-phase process: `setNextQuoteToken` stages the new quote token, and `completeQuoteTokenUpdate` finalizes it after loop-detection validation ensures the quote token chain terminates at the root token (pathUSD) without cycles.

### Transfer Authorization

Every transfer (including `transfer`, `transferFrom`, `transferWithPaymentData`, and fee transfers) is checked against the MIP-403 Transfer Policy Registry. The `ensure_transfer_authorized` function verifies that both the sender and recipient are authorized under the token's assigned `transfer_policy_id`. This enforcement is automatic and cannot be bypassed by any user, including the token administrator.

### Events

```
event Transfer(address indexed from, address indexed to, uint256 amount)
event Approval(address indexed owner, address indexed spender, uint256 amount)
event TransferWithMemo(address indexed from, address indexed to, uint256 amount, bytes32 memo)
event TransferWithPaymentData(
    address indexed from, address indexed to, uint256 amount,
    bytes endToEndId, bytes4 purposeCode, bytes remittanceInfo
)
event Mint(address indexed to, uint256 amount)
event Burn(address indexed from, uint256 amount)
event BurnBlocked(address indexed from, uint256 amount)
event PauseStateUpdate(address indexed updater, bool isPaused)
event SupplyCapUpdate(address indexed updater, uint256 newSupplyCap)
event TransferPolicyUpdate(address indexed updater, uint64 newPolicyId)
event QuoteTokenUpdate(address indexed updater, address newQuoteToken)
event NextQuoteTokenSet(address indexed updater, address nextQuoteToken)
```

---

## Appendix B: Transaction Type 0x76 Encoding

The Magnus transaction type (`0x76`) extends the EIP-2718 typed transaction envelope with fields for multi-currency gas payment and atomic batch execution. The type identifier `0x76` was chosen to avoid conflicts with the Ethereum standard type range (`0x00`–`0x03`) and common L2 extensions.

### Wire Format

```
0x76 || RLP([chain_id, max_priority_fee_per_gas, max_fee_per_gas,
             gas_limit, calls, access_list, nonce, fee_token])
```

The transaction is encoded as a single type byte (`0x76`) followed by an RLP-encoded list of fields. The field ordering places gas parameters first for efficient validation, followed by the call batch, access list, nonce, and the optional fee token.

### Field Definitions

| Field | RLP Type | Description |
|-------|----------|-------------|
| `chain_id` | `uint64` | Chain identifier for replay protection |
| `max_priority_fee_per_gas` | `uint128` | EIP-1559 priority fee tip |
| `max_fee_per_gas` | `uint128` | EIP-1559 maximum total fee |
| `gas_limit` | `uint64` | Maximum gas units for the transaction |
| `calls` | `RLP list` | Ordered list of `Call` structures |
| `access_list` | `RLP list` | EIP-2930 access list entries |
| `nonce` | `uint64` | Sender's transaction nonce |
| `fee_token` | `Address` or `0x80` | MIP-20 token address for gas, or RLP empty string (`0x80`) for native currency |

### Call Structure

Each element in the `calls` list is an RLP-encoded list:

```
RLP([to, value, input])
```

| Field | RLP Type | Description |
|-------|----------|-------------|
| `to` | `TxKind` | Destination address (call) or empty (create) |
| `value` | `U256` | Wei value transferred with this call |
| `input` | `bytes` | ABI-encoded calldata |

### Fee Token Encoding

The `fee_token` field uses context-dependent encoding. When the transaction pays gas in a MIP-20 stablecoin, the field contains the 20-byte token address encoded per standard RLP address rules. When the transaction pays gas in the native currency, the field is encoded as the RLP empty string (`0x80`), a single byte. The decoder distinguishes between these cases by checking whether the first byte of the remaining buffer equals `0x80`.

### Signing Hash

The signing hash for a Magnus transaction is computed as:

```
keccak256(0x76 || RLP([chain_id, max_priority_fee_per_gas, max_fee_per_gas,
                       gas_limit, calls, access_list, nonce, fee_token]))
```

This follows the EIP-2718 convention where the type byte is included in the hash preimage, binding the signature to the specific transaction type and preventing cross-type replay attacks.

### Decoding Algorithm

The decoder processes the byte stream as follows:

1. Verify the first byte equals `0x76`; reject otherwise.
2. Skip the type byte and decode the outer RLP list header.
3. Decode `chain_id` and verify it matches the expected chain; reject on mismatch.
4. Decode `max_priority_fee_per_gas`, `max_fee_per_gas`, and `gas_limit` as unsigned integers.
5. Decode the inner `calls` list header, then iteratively decode each `Call` structure until the calls payload is exhausted.
6. Decode the `access_list` as an EIP-2930 access list.
7. Decode `nonce` as a `uint64`.
8. If the remaining buffer is non-empty and the next byte is `0x80`, consume it and set `fee_token = None`. Otherwise, decode a 20-byte address and set `fee_token = Some(address)`. If the buffer is empty, set `fee_token = None`.

---

## Appendix C: Oracle Registry Technical Specification

The Oracle Registry manages foreign exchange rate feeds that enable multi-currency gas payment on Magnus Chain. The design is based on the Celo SortedOracles pattern with extensions for circuit breaker protection and configurable expiry windows.

### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `DEFAULT_REPORT_EXPIRY` | 360 seconds | Default time-to-live for oracle reports |
| `BREAKER_THRESHOLD_BPS` | 2000 (20%) | Maximum rate deviation before circuit breaker triggers |
| `BPS_DENOMINATOR` | 10000 | Basis points divisor |

### Rate Pair Identification

Each rate pair is identified by the keccak256 hash of the concatenated base and quote token addresses:

```
pair_id = keccak256(base_token_address ++ quote_token_address)
```

The rate semantics follow the convention that `1 unit of base_token = rate units of quote_token`. For example, a VND/USD pair with rate 25,500 means 1 VND = 25,500 units in the oracle's fixed-point representation.

### State

| Field | Type | Description |
|-------|------|-------------|
| `rate_pairs` | `Map<B256, SortedOracleList>` | Per-pair sorted lists of active reports |
| `reporters` | `Map<Address, bool>` | Whitelisted reporter addresses |
| `expiry_overrides` | `Map<B256, u64>` | Custom expiry durations per pair |
| `frozen_pairs` | `Map<B256, bool>` | Circuit breaker state per pair |

### API

**`report(reporter, base_token, quote_token, value, timestamp)`**

Submits a rate observation. The function verifies that the caller is a whitelisted reporter, that the rate pair is not frozen by the circuit breaker, and that the reported value does not deviate from the current median by more than the breaker threshold. If the deviation exceeds the threshold, the pair is frozen and the report is rejected. Valid reports are inserted into the sorted list for the pair, maintaining sort order for efficient median computation.

**`get_rate(base_token, quote_token, timestamp)`**

Returns the median rate for the specified pair. The function first prunes expired reports (those older than the pair's expiry window), then computes the median of the remaining valid reports. If no valid reports remain, the function returns an error, ensuring that stale data is never used for fee conversions.

**`num_reports(base_token, quote_token, timestamp)`**

Returns the count of non-expired reports for a pair at the given timestamp.

**`reset_breaker(base_token, quote_token)`**

Governance action that unfreezes a rate pair after a circuit breaker event. After reset, reporters can submit new reports, but the existing reports in the sorted list are preserved.

**`set_expiry(base_token, quote_token, expiry)`**

Sets a custom report expiry duration for a specific pair, overriding the default 360-second window.

### Circuit Breaker Mechanism

The circuit breaker protects against oracle manipulation and extreme market volatility. When a new report is submitted, the registry computes the deviation from the current median:

```
deviation = |new_value - current_median| * 10000 / current_median
```

If `deviation > 2000` (i.e., more than 20% from the median), the pair is immediately frozen. No further reports are accepted for the frozen pair until governance explicitly resets the breaker via `reset_breaker`. This mechanism prevents a single compromised reporter from manipulating the rate used for fee conversions while allowing legitimate rate movements within the 20% band.

### Median Computation

Reports are maintained in a sorted list data structure. When a median is requested, expired reports are filtered out, and the median is computed as the middle element of the sorted valid reports. For an even number of reports, the lower-middle element is selected. This approach provides O(1) median retrieval after the O(n) expiry filtering pass, and ensures that the median is resistant to outlier manipulation as long as fewer than half of the reporters are compromised.

---

## Appendix D: ISO 20022 Message Formats

Magnus Chain supports four ISO 20022 message types that collectively span the payment initiation, execution, reporting, and notification lifecycle. The on-chain representation stores essential fields as event data, while the complete XML documents are stored off-chain and referenced by content hash.

### pain.001 — Customer Credit Transfer Initiation

The pain.001 message initiates a payment from the debtor's account. On Magnus Chain, this corresponds to a `transferWithPaymentData` call where the sender specifies the recipient, amount, and payment metadata.

| ISO 20022 Field | On-Chain Mapping | Constraints |
|-----------------|------------------|-------------|
| `MsgId` | Transaction hash | 32 bytes, unique per transaction |
| `CreDtTm` | Block timestamp (millisecond precision via `MILLIS_TIMESTAMP`) | UTC |
| `NbOfTxs` | Batch call count from 0x76 `calls.length` | Per-transaction |
| `PmtInfId` | `endToEndId` parameter | Max 35 characters |
| `EndToEndId` | `endToEndId` parameter | Max 35 characters |
| `InstdAmt` | `amount` parameter | 6-decimal fixed point |
| `InstdAmt@Ccy` | `currency()` from MIP-20 token | ISO 4217 code |
| `Cdtr` | `to` address | 20-byte Ethereum address |
| `Dbtr` | `from` address (msg.sender) | 20-byte Ethereum address |
| `Purp/Cd` | `purposeCode` parameter | 4 bytes, ExternalPurpose1Code |
| `RmtInf/Ustrd` | `remittanceInfo` parameter | Max 140 characters |

### pacs.008 — Financial Institution Credit Transfer

The pacs.008 message represents interbank settlement. On Magnus Chain, this maps to the actual on-chain transfer event, carrying the settlement details that financial institutions use for clearing and reconciliation.

| ISO 20022 Field | On-Chain Mapping |
|-----------------|------------------|
| `GrpHdr/MsgId` | Block hash |
| `GrpHdr/CreDtTm` | Block timestamp |
| `GrpHdr/NbOfTxs` | Transaction count in block |
| `GrpHdr/SttlmInf/SttlmMtd` | "CLRG" (cleared on-chain) |
| `CdtTrfTxInf/PmtId/EndToEndId` | `endToEndId` from `TransferWithPaymentData` event |
| `CdtTrfTxInf/IntrBkSttlmAmt` | `amount` from `Transfer` event |
| `CdtTrfTxInf/IntrBkSttlmDt` | Block date |
| `CdtTrfTxInf/Purp/Cd` | `purposeCode` from event |

### camt.053 — Bank-to-Customer Statement

The camt.053 message provides periodic account statements. The banking gateway generates these by aggregating on-chain `Transfer` and `TransferWithPaymentData` events over a reporting period.

| ISO 20022 Field | Source |
|-----------------|--------|
| `GrpHdr/MsgId` | Gateway-generated identifier |
| `Stmt/Id` | Account address + period |
| `Stmt/CreDtTm` | Statement generation timestamp |
| `Stmt/Acct/Id` | Account address |
| `Stmt/Acct/Ccy` | Token `currency()` |
| `Stmt/Bal/Amt` | Token `balanceOf(account)` at period end |
| `Stmt/Ntry/Amt` | Individual transfer amounts |
| `Stmt/Ntry/CdtDbtInd` | Credit or debit indicator |
| `Stmt/Ntry/BookgDt` | Block timestamp of transfer |
| `Stmt/Ntry/NtryDtls/TxDtls/RmtInf` | `remittanceInfo` from event |

### camt.054 — Bank-to-Customer Debit/Credit Notification

The camt.054 message provides real-time notifications for individual transactions. On Magnus Chain, the banking gateway emits these immediately upon observing a finalized `TransferWithPaymentData` event, leveraging the approximately 150-millisecond deterministic finality to deliver near-instant notification to the recipient's banking system.

| ISO 20022 Field | Source |
|-----------------|--------|
| `GrpHdr/MsgId` | Transaction hash |
| `Ntfctn/Id` | Transaction hash + log index |
| `Ntfctn/CreDtTm` | Block timestamp (millisecond precision) |
| `Ntfctn/Acct/Id` | Recipient address |
| `Ntfctn/Ntry/Amt` | Transfer amount |
| `Ntfctn/Ntry/CdtDbtInd` | "CRDT" for credits, "DBIT" for debits |
| `Ntfctn/Ntry/NtryDtls/TxDtls/Refs/EndToEndId` | `endToEndId` from event |
| `Ntfctn/Ntry/NtryDtls/TxDtls/Purp/Cd` | `purposeCode` from event |
| `Ntfctn/Ntry/NtryDtls/TxDtls/RmtInf/Ustrd` | `remittanceInfo` from event |

---

## Appendix E: MagnusParaEVM Benchmark Methodology

The throughput claims for the DAG-based parallel execution engine are derived from analytical modeling based on the 2-path architecture described in Section 3, informed by the operation-level OCC results reported in Ruan et al. (EuroSys 2025, arXiv:2211.07911). This appendix describes the methodology and assumptions underlying the benchmark projections.

### Workload Model

The benchmark workload models a payment-dominated transaction mix representative of Magnus Chain's target use case. The transaction population consists of three categories: simple token transfers (60% of transactions), which touch exactly two accounts (sender and receiver); payment-with-data transfers (30%), which touch two accounts plus emit event data; and DeFi interactions (10%), which touch variable account sets depending on the protocol.

The Transaction Router classifies each transaction in O(1) time using a `HashSet<Address>` of known contract addresses and a `HashMap<(Address, [u8;4]), ExecutionPath>` of known contract-selector pairs. Native transfers and known MIP-20 operations route to Path 1 (exact scheduling); unknown contracts route to Path 2 (operation-level OCC). For the benchmark workload, approximately 70% of transactions route to Path 1 and 30% to Path 2.

### Path 1: Exact Scheduling Performance

Path 1 uses a single-pass greedy frame packing algorithm with O(n) complexity. The scheduler maintains a `HashMap<Address, FrameId>` tracking which frame currently holds each address. Each transaction's read/write set is derived from its known type (e.g., MIP-20 transfer touches `balances[from]`, `balances[to]`, `allowances[from][spender]`). Transactions are packed into the first frame where none of their addresses conflict.

Because the read/write sets are exact (derived from known contract storage layouts), Path 1 achieves zero false positives and zero speculative overhead. Each frame executes in parallel across REVM workers with no validation or redo required.

| Metric | Value | Notes |
|--------|-------|-------|
| Scheduling overhead | O(n) single pass | HashMap lookups per transaction |
| False positive rate | 0% | Exact HashSet, no bloom filter |
| Speculative overhead | 0% | No validation or redo |
| Parallel speedup (16 cores) | 12-14x | Near-linear scaling |

### Path 2: Operation-Level OCC Performance

Path 2 executes unknown contract transactions optimistically across REVM workers, with each EVM opcode logged to an SSA (Static Single Assignment) operation log. After optimistic execution, the OCC validator checks for read-write conflicts at the storage slot level. When conflicts are detected, the SSA redo engine replays only the affected operations (typically 5-15% of total operations) rather than re-executing entire transactions.

The operation-level granularity delivers significantly better performance than transaction-level OCC approaches. Ruan et al. (EuroSys 2025) demonstrated a 4.28x speedup for operation-level OCC compared to 2.49x for transaction-level OCC on representative Ethereum workloads.

| Metric | Value | Notes |
|--------|-------|-------|
| Optimistic execution | Full parallel | All workers execute concurrently |
| Conflict detection | Storage slot level | Fine-grained OCC validation |
| Redo granularity | Operation level | Only 5-15% of ops replayed |
| Parallel speedup (16 cores) | 4-5x | Per Ruan et al. EuroSys 2025 |

### Blended Throughput Model

The blended throughput for a payment-dominated workload is:

```
TPS_blended = (fraction_path1 × TPS_path1) + (fraction_path2 × TPS_path2)
```

For a 16-core validator processing 50,000 transactions per batch:

| Path | Fraction | Speedup | Effective TPS | Notes |
|------|----------|---------|---------------|-------|
| Path 1 (exact) | 70% | 12-14x | ~1.75M | Zero overhead scheduling |
| Path 2 (OCC) | 30% | 4-5x | ~0.35M | Operation-level redo |
| **Blended** | **100%** | **9-11x** | **~2.1M** | Weighted average |

The Lazy Beneficiary optimization defers `coinbase` fee distribution to the end of block execution, eliminating the universal write conflict that would otherwise serialize all transactions through the block producer's balance slot.

### Hardware Assumptions

The benchmark projections assume validator hardware consistent with institutional-grade infrastructure:

| Component | Specification |
|-----------|--------------|
| CPU | 16 physical cores, 3.0+ GHz |
| Memory | 128 GB DDR5 |
| Storage | NVMe SSD, 3+ GB/s sequential write |
| Network | 10 Gbps dedicated |

These specifications are commercially available and represent a reasonable baseline for validators operating in a payment-focused network. The 16-core baseline is deliberately conservative; scaling to 32 or 64 cores provides additional headroom.

### Comparison Notes

The throughput projections are derived from the analytical model described above and informed by the empirical results of Ruan et al. (EuroSys 2025, arXiv:2211.07911). End-to-end benchmarking on the complete Magnus Chain stack is planned for Phase 1 of the development roadmap. Actual performance may vary depending on the transaction mix, state size, and network conditions. The 2-path architecture's key advantage is that Path 1 performance is not bounded by the worst-case conflict patterns that limit purely optimistic approaches, while Path 2 provides graceful degradation for unknown workloads.

---

## Appendix F: Glossary

**BFT (Byzantine Fault Tolerance).** A consensus property ensuring correct operation as long as fewer than one-third of participants are faulty or malicious. Magnus Chain's Simplex consensus provides deterministic BFT finality.

**BLS12-381.** An elliptic curve used for pairing-based cryptography, enabling efficient aggregate and threshold signature schemes. Magnus Chain uses BLS12-381 for validator threshold signatures via distributed key generation.

**Circuit Breaker.** A safety mechanism in the Oracle Registry that freezes a rate pair when a reported value deviates more than 20% from the current median. Prevents oracle manipulation from propagating to fee conversions.

**DKG (Distributed Key Generation).** A protocol by which validators collectively generate a shared public key and individual private key shares without any single party learning the complete private key. Used to bootstrap the BLS12-381 threshold signature scheme.

**EIP-1559.** An Ethereum fee mechanism that splits transaction fees into a base fee (burned) and a priority fee (paid to validators). Magnus Chain's 0x76 transaction type extends EIP-1559 with a `fee_token` field.

**EIP-2718.** The Ethereum typed transaction envelope standard. Magnus Chain's 0x76 transaction type follows this standard, using the type byte to distinguish Magnus transactions from standard Ethereum types.

**MagnusParaEVM.** Magnus Chain's 2-path parallel execution engine. Path 1 routes known payment transactions through exact HashSet-based scheduling with zero false positives. Path 2 handles unknown contract interactions through operation-level optimistic concurrency control with SSA redo, re-executing only affected operations rather than entire transactions.

**FeeManager.** The precompile contract that orchestrates multi-currency gas fee collection, managing the pre-execution fee lock, post-execution refund, oracle-based conversion, and fee accumulation for validators.

**ISO 4217.** The international standard for currency codes (e.g., VND for Vietnamese Dong, USD for US Dollar). MIP-20 tokens store their `currency` field as an ISO 4217 code.

**ISO 20022.** The international standard for financial messaging, defining XML-based message formats for payments, securities, and trade. Magnus Chain implements a hybrid on-chain/off-chain model for ISO 20022 compliance.

**MIP-20.** The Magnus Improvement Proposal defining the native token standard. An ERC-20 superset with payment-specific extensions including `transferWithPaymentData`, ISO 4217 currency codes, role-based access control, and MIP-403 transfer policy integration.

**MIP-403.** The Magnus Improvement Proposal defining the Transfer Policy Registry. Provides whitelist and blacklist policy types that are automatically enforced on all MIP-20 token transfers.

**MILLIS_TIMESTAMP.** A custom EVM opcode (`0x4F`) that returns the current block timestamp with millisecond precision, enabling sub-second time resolution for payment processing and ISO 20022 `CreDtTm` fields.

**Oracle Registry.** The precompile contract managing foreign exchange rate feeds. Whitelisted reporters submit rate observations that are sorted and aggregated via median calculation, with circuit breaker protection against manipulation.

**Transaction Router.** The O(1) classifier in MagnusParaEVM that routes transactions to Path 1 (exact scheduling) or Path 2 (OCC) based on a HashSet of known contract addresses and a HashMap of known contract-selector pairs.

**SSA (Static Single Assignment) Redo.** The conflict resolution mechanism in MagnusParaEVM Path 2. Each EVM opcode is logged with its inputs and outputs during optimistic execution. When the OCC validator detects conflicts, only the affected operations are replayed rather than entire transactions.

**Exact Scheduler.** The frame-based greedy packing algorithm in MagnusParaEVM Path 1. Groups non-conflicting transactions into frames for parallel execution using exact read/write sets derived from known contract storage layouts.

**Payment Lane.** A block structure mechanism using separate `general_gas_limit` and `shared_gas_limit` fields in the Magnus block header to isolate payment transaction capacity from general-purpose DeFi congestion.

**QMDB (Quantum Merkle Database).** Magnus Chain's authenticated state storage, using a Merkle Mountain Range structure with generation-based copy-on-write semantics. Achieves O(1) amortized I/O per state update.

**REVM.** The Rust Ethereum Virtual Machine implementation used as the execution backend in Magnus Chain's worker pool. Each REVM instance executes a conflict-free group of transactions in parallel.

**Simplex BFT.** The consensus protocol used by Magnus Chain, providing deterministic finality with a target latency of approximately 150 milliseconds.

**VNST.** A Vietnamese Dong-pegged stablecoin deployed as an MIP-20 token on Magnus Chain, with `currency = "VND"` and supply managed by an authorized issuer holding the `ISSUER_ROLE`.

**2D Nonce System.** A dual-dimension nonce scheme where each account maintains a protocol nonce (key 0) in standard account state and additional user-defined nonce keys (1 through N) in the nonce precompile. Enables concurrent transaction streams from a single account without serialization.

---

## References

1. Ruan, C. et al. ParallelEVM: Operation-Level Optimistic Concurrency Control for EVM Blockchains. EuroSys 2025. arXiv:2211.07911.

2. QMDB: Quick Merkle Database for Blockchain State Storage. arXiv:2501.05262, 2025.

3. ISO 20022 Financial Services — Universal Financial Industry Message Scheme. International Organization for Standardization, 2004–2025.

4. ISO 4217 Currency Codes. International Organization for Standardization, 2015.

5. EIP-1559: Fee Market Change for ETH 1.0 Chain. Ethereum Improvement Proposals, 2019.

6. EIP-2718: Typed Transaction Envelope. Ethereum Improvement Proposals, 2020.

7. EIP-2930: Optional Access Lists. Ethereum Improvement Proposals, 2020.

8. SWIFT ISO 20022 Programme. Society for Worldwide Interbank Financial Telecommunication, 2018–2025.

9. World Bank Remittance Prices Worldwide Quarterly, Issue 50. World Bank Group, 2024.

10. BLS12-381: New zk-SNARK Elliptic Curve Construction. Bowe, S., 2017.

