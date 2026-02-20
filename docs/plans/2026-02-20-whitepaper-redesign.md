# Magnus Chain Whitepaper Redesign

**Date:** February 20, 2026
**Status:** Approved for Implementation
**Author:** Architecture Team

---

## Executive Summary

The Magnus Chain whitepaper is being redesigned to reflect the final architecture decision: **Grevm 2.1 + Commonware** (single execution path) instead of the original MagnusParaEVM 2-path hybrid design.

### Key Changes

1. **Architecture Simplification**
   - Remove: MagnusParaEVM 2-path (Path 1 exact scheduling + Path 2 OCC)
   - Add: DAG-based parallel execution engine (Grevm-inspired, no brand mention)
   - Keep: Payment lanes (Tempo mechanism, dual gas limits)

2. **Performance Targets (Conservative)**
   - Original: 2M+ TPS on 16 cores
   - Revised: 700K-1M TPS on 16-32 cores

3. **Structure Change**
   - Original: 4-pillar structure (Execution → Primitives → ISO 20022 → Infrastructure)
   - New: Problem-Solution-Proof (The Gap → The Solution → Technical Architecture)

4. **Content Prioritization**
   - Medium detail: Payment primitives + Parallel execution
   - High level: Everything else (infrastructure, security)

5. **Removed Sections**
   - Market Opportunity (Vietnam beachhead, SE Asia expansion)
   - Development Roadmap

6. **Brand Name Policy**
   - Don't mention: Grevm, Tempo, Commonware (except in References)
   - Describe: The mechanisms themselves (DAG-based execution, dual gas limits, MMR storage)

---

## Design Rationale

### Why Redesign?

The original whitepaper was written before the final architecture decision. After comprehensive research (see `2026-02-20-magnus-parallel-evm-final-design.md`), the team selected **Grevm 2.1 + Commonware** as the execution stack for:

1. **Production readiness** — Grevm deployed on Gravity Chain, Commonware battle-tested
2. **Simplicity** — Single execution path vs 3-tier hybrid
3. **Realistic performance** — 700K-1M TPS achievable on 16-32 cores (vs optimistic 2M on 16 cores)
4. **Never drops transactions** — Critical for banking settlement guarantees
5. **Lower maintenance** — One codebase vs multiple paths

### Why Problem-Solution-Proof Structure?

The new structure (Alternative 1 from brainstorming) provides:

1. **Better narrative flow** — Problem definition → What we built → How it works
2. **Mixed audience support** — Executives can stop after Part II, engineers dive into Part III
3. **Clear value proposition** — "The Gap" section motivates every solution in Part II

---

## New Whitepaper Structure

```
1. INTRODUCTION
   1.1 The Broken State of Cross-Border Payments
   1.2 Why Existing Blockchains Fail for Regulated Payments
   1.3 The ISO 20022 Convergence
   1.4 The Magnus Chain Thesis

2. DESIGN PHILOSOPHY
   (Unchanged — 4 principles)

3. PART I: THE PAYMENT INFRASTRUCTURE GAP
   3.1 Throughput Bottleneck
   3.2 Compliance Void
   3.3 Multi-Currency Barrier
   3.4 Interoperability Failure
   3.5 Settlement Risk

4. PART II: THE MAGNUS SOLUTION
   4.1 Payment Primitives (MEDIUM DETAIL)
   4.2 Banking Integration (HIGH LEVEL)
   4.3 Scale and Performance (MEDIUM DETAIL)

5. PART III: TECHNICAL ARCHITECTURE
   5.1 Execution Layer (HIGH LEVEL)
   5.2 Infrastructure Foundation (HIGH LEVEL)
   5.3 Security Model (HIGH LEVEL)

6. COMPETITIVE ANALYSIS AND BENCHMARKS
   6.1 Platform Comparison
   6.2 Transaction Cost Analysis
   6.3 Throughput Benchmarks

7. SECURITY AND RESILIENCE

8. APPENDICES
   A. MIP-20 Token Standard Specification
   B. MIP-403 Policy Types
   C. ISO 20022 Message Mappings
   D. Performance Benchmark Methodology
   E. Glossary
   F. References
```

---

## Section-by-Section Content Guide

### Section 1-2: Introduction + Design Philosophy

**Status:** Mostly unchanged, minor updates

**Changes needed:**
- Update Section 1.4 thesis statement to replace "MagnusParaEVM 2-path" with "DAG-based parallel execution"
- Update Abstract to replace "2 million TPS" with "700,000+ TPS"
- Replace "Path 1/Path 2" references with "DAG-based parallel execution + payment lanes"

---

### Section 3: Part I - The Payment Infrastructure Gap

**Status:** NEW SECTION

**Purpose:** Define the 5-dimensional problem that Magnus solves

**Content:** (See full drafted content below)

**Subsections:**
- 3.1 Throughput Bottleneck (Ethereum 15 TPS vs banking needs 5K-65K TPS)
- 3.2 Compliance Void (no protocol-level KYC, policies, payment data)
- 3.3 Multi-Currency Barrier (forced crypto holdings, volatile gas)
- 3.4 Interoperability Failure (no ISO 20022, SWIFT incompatible)
- 3.5 Settlement Risk (probabilistic finality unsuitable for banking)

**Length:** ~2,500 words (500 words per subsection)

---

### Section 4: Part II - The Magnus Solution

**Status:** MAJOR REWRITE

**Purpose:** Present what Magnus built to solve the gap (medium detail on key features)

**Content:** (See full drafted content below)

**4.1 Payment Primitives (MEDIUM DETAIL)**
- MIP-20 Token Standard (ISO 4217, payment data fields, role-based minting)
- transferWithPaymentData (endToEndId, purposeCode, remittanceInfo)
- Multi-Currency Gas Fees (Oracle Registry, FeeManager, 0x76 transactions)
- Transfer Policy Registry MIP-403 (whitelist, blacklist, freeze, time-lock)

**4.2 Banking Integration (HIGH LEVEL)**
- ISO 20022 hybrid storage (on-chain fields, off-chain XML)
- Banking gateway (SWIFT/NAPAS connectors, bidirectional)
- KYC Registry (tiered verification, FATF-aligned)

**4.3 Scale and Performance (MEDIUM DETAIL)**
- DAG-Based Parallel Execution (4-step: hint → DAG → task groups → execute)
- Payment Lanes (dual gas limits, TIP-20 prefix classification, 5-section blocks)
- Performance: 4× speedup → 700K-1M TPS

**Length:** ~3,500 words (2,000 for primitives, 500 for banking, 1,000 for execution)

**Key writing guidelines:**
- Don't say "Grevm" — say "DAG-based parallel execution engine"
- Don't say "Tempo payment lanes" — say "payment lanes use dual gas limits"
- Don't say "Commonware QMDB" — say "MMR-based authenticated storage"
- Provide concrete examples (Vietnamese worker remittance, payroll batch)
- Explain WHY each feature matters for banking

---

### Section 5: Part III - Technical Architecture

**Status:** MAJOR REWRITE

**Purpose:** Technical details for engineers (high level, references for depth)

**Content:** (See full drafted content below)

**5.1 Execution Layer (HIGH LEVEL)**
- AOT compilation (LLVM-based, 1.5-2× speedup for hot contracts)
- Async pipeline (5-stage, execution ∥ merkleization)
- Concurrent state cache (lock-free, block-tagged, prevents degradation)

**5.2 Infrastructure Foundation (HIGH LEVEL)**
- Consensus: Simplex BFT (200ms block, 300ms finality, deterministic)
- Storage: MMR structure (parallel merkleization 4-6×, append-only)
- Cryptography: BLS12-381 (threshold signatures, DKG ceremonies)
- Modularity: 46 Rust crates, trait-based separation

**5.3 Security Model (HIGH LEVEL)**
- Consensus security (BFT assumptions, threshold signatures)
- Oracle manipulation resistance (median, circuit breaker, expiry)
- Payment lane isolation (DoS resistance, economic attack prevention)
- Compliance enforcement (protocol-level, no bypass paths)
- Cryptographic security (128-bit classical, verifiable secret sharing)

**Length:** ~2,000 words (500 per subsection, 400 for each infrastructure component)

**Key writing guidelines:**
- Brief technical descriptions (50-100 words per component)
- Reference appendices or external docs for full details
- Focus on "what" not "how" (save implementation details for technical docs)

---

### Section 6: Competitive Analysis and Benchmarks

**Status:** UPDATE NUMBERS

**Changes needed:**
- Update throughput: 500K+ → 700K+ TPS
- Update table: "MagnusParaEVM 2-path" → "DAG parallel EVM"
- Update Section 6.3 benchmark methodology to reference DAG execution (not FAFO)
- Remove ParallelEVM paper citation (operation-level OCC not used)

**Content:** (See full drafted content in previous messages)

---

### Section 7: Security and Resilience

**Status:** MINOR UPDATES

**Changes:**
- Reference Section 5.3 security model
- Add operational security (validator set, network attacks, recovery)
- Remove MagnusParaEVM-specific security analysis

**Content:** (See full drafted content in previous messages)

---

### Section 8: Appendices

**Status:** UPDATE REFERENCES

**Appendix A: MIP-20 Specification** (unchanged)

**Appendix B: MIP-403 Policy Types** (unchanged)

**Appendix C: ISO 20022 Mappings** (unchanged)

**Appendix D: Performance Benchmark Methodology** (REPLACE)
- Remove: FAFO benchmark methodology
- Add: DAG-based parallel execution performance model
- Reference: Gravity Reth production data (41K TPS baseline)
- Banking optimizations: static transpilation (2×), pre-scheduling (1.2×), async storage (1.4×)

**Appendix E: Glossary** (UPDATE)
- Remove: MagnusParaEVM, Transaction Router, SSA Redo, Exact Scheduler, ParallelEVM
- Add: DAG Execution, Task Groups, Payment Lanes, MMR Storage, Simplex Consensus
- Keep: All payment primitive terms (MIP-20, MIP-403, Oracle Registry, etc.)

**Appendix F: References** (UPDATE)
- Remove: [1] ParallelEVM paper (arXiv:2211.07911)
- Add:
  - [1] DAG-Based Parallel Execution for EVM Blockchains. Gravity Chain, 2024.
  - [2] Payment Lanes for Blockspace Reservation. Tempo Labs, 2025.
  - [3] Quick Merkle Database: MMR-Based Storage. Commonware, 2025.
  - [4] Simplex: Byzantine Fault Tolerant Consensus. Commonware, 2025.

---

## Content Prioritization (Medium vs High Level)

### TIER 1: Medium Detail (200-300 words per component)

**Why:** These are the differentiators that make Magnus valuable for banking

1. **MIP-20 Token Standard**
   - ISO 4217 currency codes
   - transferWithPaymentData fields
   - Role-based minting (ISSUER_ROLE, supply_cap)
   - Concrete examples of usage

2. **Multi-Currency Gas Fees**
   - Oracle Registry (median aggregation, circuit breaker)
   - FeeManager (pre-lock, conversion, refund)
   - 0x76 transaction type
   - User flow example (Vietnamese user pays in VNST)

3. **MIP-403 Transfer Policies**
   - 4 policy types (whitelist, blacklist, freeze, time-lock)
   - Protocol-level enforcement (embedded in _transfer)
   - Access control model
   - Regulatory compliance use cases

4. **DAG-Based Parallel Execution**
   - 4-step process (hint generation → DAG construction → task groups → execute)
   - WCC partitioning (independent transaction groups)
   - Task groups (sequential execution for hot accounts)
   - 4× speedup mechanism

5. **Payment Lanes**
   - Dual gas limits (gas_limit vs general_gas_limit)
   - Transaction classification (TIP-20 prefix 0x20c0)
   - 5-section block structure
   - QoS guarantee explanation

### TIER 2: High Level (50-100 words per component)

**Why:** Supporting technical details, just enough to show completeness

- ISO 20022 integration (hybrid storage, gateway, messages)
- KYC Registry (tiered verification)
- AOT compilation (LLVM, 1.5-2× speedup)
- Async pipeline (5-stage, execution ∥ merkleization)
- State cache (concurrent, block-tagged)
- Simplex consensus (200ms/300ms, deterministic)
- MMR storage (parallel merkleization 4-6×)
- BLS signatures (threshold, DKG)
- Security model (all 5 layers)

---

## Writing Style Guidelines

### Academic/Technical Whitepaper Standards

1. **No brand name mentions in body text**
   - ❌ "We use Grevm from Gravity Chain"
   - ✅ "DAG-based parallel execution engine achieves 4× speedup"
   - Citations: References section only

2. **Describe mechanisms, not implementations**
   - ❌ "Grevm's WCC partitioning with lock-free cursor"
   - ✅ "Transactions are partitioned into weakly connected components that execute independently"

3. **Concrete examples for abstract concepts**
   - MIP-20: "A Vietnamese dong stablecoin stores `currency = \"VND\"`"
   - Payment lanes: "A payroll batch of 500 transfers gets priority block space"
   - Multi-currency gas: "User holding VNST pays fees without acquiring ETH"

4. **Quantify performance claims**
   - ✅ "4× speedup on 16-core hardware for payment workloads"
   - ✅ "700,000 transactions per second on 16-32 cores"
   - ❌ "Extremely fast" or "Highly scalable"

5. **Cite prior art in References**
   - DAG execution → Reference Gravity Chain paper
   - Payment lanes → Reference Tempo design
   - MMR storage → Reference Commonware docs
   - Simplex consensus → Reference Commonware consensus paper

---

## Implementation Plan

### Step 1: Update Existing Sections (Week 1)

**Files to modify:**
- `docs/whitepaper/magnus-chain-whitepaper.md`

**Sections to update:**
1. Abstract (performance claims, architecture description)
2. Section 1.4 (thesis statement)
3. Section 2 (design philosophy references)

**Time estimate:** 2-3 hours

---

### Step 2: Write New Section 3 (Part I: The Gap) (Week 1)

**Content:** Draft provided in this design doc

**Subsections:**
- 3.1 Throughput Bottleneck (~500 words)
- 3.2 Compliance Void (~500 words)
- 3.3 Multi-Currency Barrier (~500 words)
- 3.4 Interoperability Failure (~500 words)
- 3.5 Settlement Risk (~500 words)

**Time estimate:** 4-5 hours

---

### Step 3: Rewrite Section 4 (Part II: The Solution) (Week 1-2)

**Content:** Draft provided in this design doc

**Subsections:**
- 4.1 Payment Primitives (~2,000 words, medium detail)
- 4.2 Banking Integration (~500 words, high level)
- 4.3 Scale and Performance (~1,000 words, medium detail)

**Time estimate:** 6-8 hours

---

### Step 4: Rewrite Section 5 (Part III: Technical Architecture) (Week 2)

**Content:** Draft provided in this design doc

**Subsections:**
- 5.1 Execution Layer (~500 words, high level)
- 5.2 Infrastructure Foundation (~1,000 words, high level)
- 5.3 Security Model (~500 words, high level)

**Time estimate:** 4-5 hours

---

### Step 5: Update Section 6 (Benchmarks) (Week 2)

**Changes:**
- Update performance table (700K+ TPS)
- Update execution model column ("DAG parallel EVM")
- Rewrite Section 6.3 with Grevm + banking optimizations model

**Time estimate:** 2-3 hours

---

### Step 6: Update Section 7 (Security) (Week 2)

**Changes:**
- Reference Section 5.3
- Add operational security subsections

**Time estimate:** 1-2 hours

---

### Step 7: Update Appendices (Week 2)

**Changes:**
- Appendix D: Replace FAFO methodology with DAG execution model
- Appendix E: Update glossary (remove MagnusParaEVM terms, add DAG terms)
- Appendix F: Update references (add Grevm, Tempo, Commonware papers)

**Time estimate:** 2-3 hours

---

### Step 8: Rebuild PDF (Week 2)

**Command:**
```bash
cd docs/whitepaper
pandoc magnus-chain-whitepaper.md -o magnus-chain-whitepaper.pdf \
  --template=template.latex \
  --pdf-engine=tectonic \
  -V geometry:margin=1in
```

**Time estimate:** 30 minutes

---

### Step 9: Review and Commit (Week 2)

**Review checklist:**
- [ ] All MagnusParaEVM references removed
- [ ] All performance claims updated (2M → 700K-1M)
- [ ] No brand names in body text (Grevm, Tempo, Commonware)
- [ ] All brand names cited in References
- [ ] Medium detail sections complete (payment primitives, execution)
- [ ] High level sections concise (infrastructure, security)
- [ ] PDF builds without errors
- [ ] Glossary updated
- [ ] References section complete

**Commit message:**
```
docs: redesign whitepaper with Grevm + Commonware architecture

- Replace MagnusParaEVM 2-path with DAG-based parallel execution
- Restructure: Problem-Solution-Proof (3 parts)
- Update performance: 2M TPS → 700K-1M TPS (conservative)
- Add payment lanes (Tempo mechanism)
- Remove market opportunity sections
- Remove roadmap section
- Update all references, glossary, benchmarks
```

**Time estimate:** 1-2 hours

---

## Total Timeline

**Estimated effort:** 20-30 hours (2.5-4 days)

**Recommended schedule:**
- Week 1: Steps 1-3 (update existing + write Part I + Part II)
- Week 2: Steps 4-9 (Part III + benchmarks + security + appendices + PDF + review)

---

## Success Criteria

### Content Quality

- [ ] Part I clearly defines the 5-dimensional problem
- [ ] Part II presents Magnus solutions with appropriate detail level
- [ ] Part III provides technical depth without overwhelming
- [ ] Payment primitives section is banking-audience friendly (examples, use cases)
- [ ] Parallel execution section is technically credible (4-step process clear)
- [ ] No unsubstantiated performance claims (all numbers justified)

### Technical Accuracy

- [ ] DAG execution description matches final design doc
- [ ] Payment lanes mechanism matches Tempo design
- [ ] Performance projections match optimization analysis (700K-1M TPS)
- [ ] All component descriptions match Commonware capabilities
- [ ] Security model complete across 5 layers

### Academic Standards

- [ ] No brand name mentions in body (except References)
- [ ] All prior art cited properly
- [ ] Concrete examples for abstract concepts
- [ ] Quantified performance claims
- [ ] Glossary complete and accurate

---

## Appendix: Full Drafted Content

### Section 3: Part I - The Payment Infrastructure Gap

[Full content provided in previous design presentation]

### Section 4: Part II - The Magnus Solution

[Full content provided in previous design presentation]

### Section 5: Part III - Technical Architecture

[Full content provided in previous design presentation]

### Section 6: Competitive Analysis and Benchmarks

[Full content provided in previous design presentation]

### Section 7: Security and Resilience

[Full content provided in previous design presentation]

---

**END OF WHITEPAPER REDESIGN DOCUMENT**
