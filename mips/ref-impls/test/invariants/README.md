# Magnus Invariants

## Stablecoin DEX

### Order Management

- **MAGNUS-DEX1**: Newly created order ID matches next order ID and increments monotonically.
- **MAGNUS-DEX2**: Placing an order escrows the correct amount - bids escrow quote tokens (rounded up), asks escrow base tokens.
- **MAGNUS-DEX3**: Cancelling an active order refunds the escrowed amount to the maker's internal balance.

### Swap Invariants

- **MAGNUS-DEX4**: `amountOut >= minAmountOut` when executing `swapExactAmountIn`.
- **MAGNUS-DEX5**: `amountIn <= maxAmountIn` when executing `swapExactAmountOut`.
- **MAGNUS-DEX6**: Swapper total balance (external + internal) changes correctly - loses exact `amountIn` of tokenIn and gains exact `amountOut` of tokenOut. Skipped when swapper has active orders (self-trade makes accounting complex).
- **MAGNUS-DEX7**: Quote functions (`quoteSwapExactAmountIn/Out`) return values matching actual swap execution.
- **MAGNUS-DEX8**: Dust invariant - each swap can at maximum increase the dust in the DEX by the number of orders filled plus the number of hops (rounding occurs at each hop, not just at hop boundaries).
- **MAGNUS-DEX9**: Post-swap dust bounded - maximum dust accumulation in the protocol is bounded and tracked via `_maxDust`.

### Balance Invariants

- **MAGNUS-DEX10**: DEX token balance >= sum of all internal user balances (the difference accounts for escrowed order amounts).

### Orderbook Structure Invariants

- **MAGNUS-DEX11**: Total liquidity at a tick level equals the sum of remaining amounts of all orders at that tick. If liquidity > 0, head and tail must be non-zero.
- **MAGNUS-DEX12**: Best bid tick points to the highest tick with non-empty bid liquidity, or `type(int16).min` if no bids exist.
- **MAGNUS-DEX13**: Best ask tick points to the lowest tick with non-empty ask liquidity, or `type(int16).max` if no asks exist.
- **MAGNUS-DEX14**: Order linked list is consistent - `prev.next == current` and `next.prev == current`. If head is zero, tail must also be zero.
- **MAGNUS-DEX15**: Tick bitmap accurately reflects which ticks have liquidity (bit set iff tick has orders).
- **MAGNUS-DEX16**: Linked list head/tail is terminal - `head.prev == None` and `tail.next == None`

### Flip Order Invariants

- **MAGNUS-DEX17**: Flip orders have valid tick constraints - for bids `flipTick > tick`, for asks `flipTick < tick`.

### Blacklist Invariants

- **MAGNUS-DEX18**: Anyone can cancel a stale order from a blacklisted maker via `cancelStaleOrder`. The escrowed funds are refunded to the blacklisted maker's internal balance.

### Rounding Invariants

- **MAGNUS-DEX19**: Divisibility edge cases - when `(amount * price) % PRICE_SCALE == 0`, bid escrow must be exact (no +1 rounding) since ceil equals floor for perfectly divisible amounts.

## FeeAMM

The FeeAMM is a constant-rate AMM used for converting user fee tokens to validator fee tokens. It operates with two fixed rates:

- **Fee Swap Rate (M)**: 0.9970 (0.30% fee) - Used when swapping user tokens to validator tokens during fee collection
- **Rebalance Rate (N)**: 0.9985 (0.15% fee) - Used when liquidity providers rebalance pools

### Liquidity Management Invariants

- **MAGNUS-AMM1**: Minting LP tokens always produces a positive liquidity amount when the deposit is valid.
- **MAGNUS-AMM2**: Total LP supply increases correctly on mint - by `liquidity + MIN_LIQUIDITY` for first mint, by `liquidity` for subsequent mints.
- **MAGNUS-AMM3**: Actor's LP balance increases by exactly the minted liquidity amount.
- **MAGNUS-AMM4**: Validator token reserve increases by exactly the deposited amount on mint.

### Burn Invariants

- **MAGNUS-AMM5**: Burn returns pro-rata amounts - `amountToken = (liquidity * reserve) / totalSupply` for both user and validator tokens.
- **MAGNUS-AMM6**: Total LP supply decreases by exactly the burned liquidity amount.
- **MAGNUS-AMM7**: Actor's LP balance decreases by exactly the burned liquidity amount.
- **MAGNUS-AMM8**: Actor receives the exact calculated token amounts on burn.
- **MAGNUS-AMM9**: Pool reserves decrease by exactly the returned token amounts on burn.

### Rebalance Swap Invariants

- **MAGNUS-AMM10**: Rebalance swap `amountIn` follows the formula: `amountIn = (amountOut * N / SCALE) + 1` (rounds up).
- **MAGNUS-AMM11**: Pool reserves update correctly - user reserve decreases by `amountOut`, validator reserve increases by `amountIn`.
- **MAGNUS-AMM12**: Actor balances update correctly - pays `amountIn` validator tokens, receives `amountOut` user tokens.

### Fee Swap Invariants

- **MAGNUS-AMM25**: Fee swap `amountOut` follows the formula: `amountOut = (amountIn * M / SCALE)` (rounds down). Output never exceeds input.
- **MAGNUS-AMM26**: Fee swap reserves update correctly - user reserve increases by `amountIn`, validator reserve decreases by `amountOut`. Verified via ghost variable tracking in `simulateFeeCollection`.

### Global Invariants

- **MAGNUS-AMM13**: Pool solvency - AMM token balances are always >= sum of pool reserves for that token.
- **MAGNUS-AMM14**: LP token accounting - Total supply equals sum of all user LP balances + MIN_LIQUIDITY (locked on first mint).
- **MAGNUS-AMM15**: MIN_LIQUIDITY is permanently locked - once a pool is initialized, total supply is always >= MIN_LIQUIDITY.
- **MAGNUS-AMM16**: Fee rates are constant - M = 9970, N = 9985, SCALE = 10000.
- **MAGNUS-AMM27**: Pool ID uniqueness - `getPoolId(A, B) != getPoolId(B, A)` for directional pool separation.
- **MAGNUS-AMM28**: No LP when uninitialized - if `totalSupply == 0`, no actor holds LP tokens for that pool.
- **MAGNUS-AMM29**: Fee conservation - `collectedFees + distributed <= totalFeesIn` (fees cannot be created from nothing).
- **MAGNUS-AMM30**: Pool initialization shape - a pool is either completely uninitialized (totalSupply=0, reserves=0) OR properly initialized with totalSupply >= MIN_LIQUIDITY. No partial/bricked states allowed.
- **MAGNUS-AMM31**: Fee double-count prevention - fees accumulate via `+=` (not overwrite), and `distributeFees` zeros the balance before transfer, preventing the same fees from being distributed twice.

### Rounding & Exploitation Invariants

- **MAGNUS-AMM17**: Mint/burn cycle should not profit the actor - prevents rounding exploitation.
- **MAGNUS-AMM18**: Small swaps should still pay >= theoretical rate.
- **MAGNUS-AMM19**: Must pay at least 1 for any swap - prevents zero-cost extraction.
- **MAGNUS-AMM20**: Reserves are always bounded by uint128.
- **MAGNUS-AMM21**: Spread between fee swap (M) and rebalance (N) prevents arbitrage - M < N with 15 bps spread.
- **MAGNUS-AMM22**: Rebalance swap rounding always favors the pool - the +1 in the formula ensures pool never loses to rounding, even when `(amountOut * N) % SCALE == 0` (exact division case).


- **MAGNUS-AMM23**: Burn rounding dust accumulates in pool - integer division rounds down, so users receive <= theoretical amount.
- **MAGNUS-AMM24**: All participants can exit with solvency guaranteed. After distributing all fees and burning all LP positions:

  - **Solvency**: AMM balance >= tracked reserves (LPs cannot extract more than exists)
  - **No value creation**: AMM balance does not exceed reserves by more than tracked dust sources
  - **MIN_LIQUIDITY preserved**: Even if all LP holders burn their entire balances pro-rata, MIN_LIQUIDITY worth of reserves remains permanently locked. This is guaranteed by the combination of:
    1. First mint locks MIN_LIQUIDITY in totalSupply but assigns it to no one (MAGNUS-AMM15)
    2. Pro-rata burn formula `(liquidity * reserve) / totalSupply` can only extract `userLiquidity / totalSupply` fraction
    3. Since `sum(userBalances) = totalSupply - MIN_LIQUIDITY`, full exit leaves `(MIN_LIQUIDITY / totalSupply) * reserves` in the pool

  Note: Fee swap dust (0.30% fee) and rebalance +1 rounding go INTO reserves and are distributed pro-rata to LPs when they burn. This is the intended fee mechanism - LPs earn revenue from fee swaps. The invariant verifies no value is created (balance ≤ reserves + tracked dust) rather than requiring dust to remain, since dust legitimately flows to LPs.

### MIP-403 Blacklist Invariants

Blacklist testing uses a simple approach: `toggleBlacklist` randomly adds/removes actors from token blacklists, and existing handlers (mint, burn, rebalanceSwap, distributeFees) naturally encounter blacklisted actors and verify `PolicyForbids` behavior.

- **MAGNUS-AMM32**: Blacklisted actors cannot receive tokens from AMM operations. Operations that would transfer tokens to a blacklisted recipient (burn, rebalanceSwap, distributeFees) revert with `PolicyForbids`. Frozen fees/LP remain intact and are not lost.
- **MAGNUS-AMM33**: Blacklisted actors cannot deposit tokens into the AMM. Mint operations from blacklisted actors revert with `PolicyForbids`.
- **MAGNUS-AMM34**: Blacklist recovery - after being removed from blacklist, validators can claim their frozen fees and LPs can burn their positions. Blacklisting is a temporary freeze, not permanent loss. Verified in the two-phase exit check: Phase 1 exits with blacklisted actors frozen, Phase 2 unblacklists all actors and verifies complete recovery.

## FeeManager

The FeeManager extends FeeAMM and handles fee token preferences and distribution for validators and users.

### Token Preference Invariants

- **MAGNUS-FEE1**: `setValidatorToken` correctly stores the validator's token preference.
- **MAGNUS-FEE2**: `setUserToken` correctly stores the user's token preference.

### Fee Distribution Invariants

- **MAGNUS-FEE3**: After `distributeFees`, collected fees for that validator/token pair are zeroed.
- **MAGNUS-FEE4**: Validator receives exactly the previously collected fee amount on distribution.

### Fee Collection Invariants

- **MAGNUS-FEE5**: Combined solvency - for each token, total pool reserves + collected fees ≤ AMM token balance.
- **MAGNUS-FEE6**: Fee swap rate M is correctly applied - fee output should always be <= fee input.

## MIP-1000: State Creation Cost (Gas Pricing)

MIP-1000 defines Magnus's gas pricing for state creation operations, charging 250,000 gas for each new state element to account for long-term storage costs.

### State Creation Invariants (GasPricing.t.sol)

Tested via `vmExec.executeTransaction()` - executes real transactions and verifies gas requirements:

- **MAGNUS-GAS1**: SSTORE to new slot costs exactly 250,000 gas.
  - Handler executes SSTORE with insufficient gas (100k) and sufficient gas (350k)
  - Invariant: insufficient gas must fail, sufficient must succeed

- **MAGNUS-GAS5**: Contract creation cost = (code_size × 1,000) + 500,000 + 250,000 (account creation).
  - Handler deploys contracts with insufficient and sufficient gas
  - Invariant: deployment must fail below threshold, succeed above

- **MAGNUS-GAS8**: Multiple new state elements charge 250k each independently.
  - Handler writes N slots (2-5) with gas for only 1 slot vs gas for N slots
  - Invariant: all N slots must not be written with gas for only 1

### Protocol-Level Invariants (Rust)

The following are enforced at the protocol level and tested in Rust:

- **MAGNUS-GAS2**: Account creation intrinsic gas (250k) → `crates/revm/src/handler.rs`
- **MAGNUS-GAS3**: SSTORE reset cost (5k) → `crates/revm/`
- **MAGNUS-GAS4**: Storage clear refund (15k) → `crates/revm/`
- **MAGNUS-GAS6**: Transaction gas cap (30M) → `crates/transaction-pool/src/validator.rs`
- **MAGNUS-GAS7**: First tx minimum gas (271k) → `crates/transaction-pool/src/validator.rs`
- **MAGNUS-GAS9-14**: Various protocol-level gas rules → `crates/revm/`

## MIP-1010: Mainnet Gas Parameters (Block Limits)

MIP-1010 defines Magnus's mainnet block gas parameters, including a 500M total block gas limit with a 30M general lane and 470M payment lane allocation.

### Block Gas Invariants (BlockGasLimits.t.sol)

Tested via `vmExec.executeTransaction()` and constant assertions:

- **MAGNUS-BLOCK1**: Block total gas limit = 500,000,000. (constant assertion)
- **MAGNUS-BLOCK2**: General lane gas limit = 30,000,000. (constant assertion)
- **MAGNUS-BLOCK3**: Transaction gas cap = 30,000,000.
  - Handler submits tx at cap (30M) and over cap (30M+)
  - Invariant: over-cap transactions must be rejected

- **MAGNUS-BLOCK4**: Base fee = 20 gwei (T1), 10 gwei (T0). (constant assertion)
- **MAGNUS-BLOCK5**: Payment lane minimum = 470M. (constant assertion)
- **MAGNUS-BLOCK6**: Max contract deployment (24KB) fits within tx gas cap.
  - Handler deploys contracts at 50-100% of max size
  - Invariant: max size deployment must succeed within tx cap

### Protocol-Level Invariants (Rust)

The following are enforced in the block builder and tested in Rust:

- **MAGNUS-BLOCK7**: Block validity rejects over-limit blocks → `crates/payload/builder/src/lib.rs`
- **MAGNUS-BLOCK8-9**: Hardfork activation rules → `crates/chainspec/`
- **MAGNUS-BLOCK10**: Shared gas limit = block_gas_limit / 10 → `crates/consensus/src/lib.rs`
- **MAGNUS-BLOCK11**: Constant base fee within epoch → `crates/chainspec/`
- **MAGNUS-BLOCK12**: General lane enforcement (30M cap) → `crates/payload/builder/src/lib.rs`

## Nonce

The Nonce precompile manages 2D nonces for accounts, enabling multiple independent nonce sequences per account identified by a nonce key.

### Nonce Increment Invariants

- **MAGNUS-NON1**: Monotonic increment - nonces only ever increase by exactly 1 per increment operation.
- **MAGNUS-NON2**: Ghost state consistency - actual nonce values always match tracked ghost state.
- **MAGNUS-NON3**: Read consistency - `getNonce` returns the correct value after any number of increments.

### Protocol Nonce Invariants

- **MAGNUS-NON4**: Protocol nonce rejection - nonce key 0 is reserved for protocol nonces and reverts with `ProtocolNonceNotSupported` when accessed through the precompile.

### Independence Invariants

- **MAGNUS-NON5**: Account independence - incrementing one account's nonce does not affect any other account's nonces.
- **MAGNUS-NON6**: Key independence - incrementing one nonce key does not affect any other nonce key for the same account.

### Edge Case Invariants

- **MAGNUS-NON7**: Large nonce key support - `type(uint256).max - 1` works correctly as a nonce key. Note: `type(uint256).max` is reserved for `TEMPO_EXPIRING_NONCE_KEY`.
- **MAGNUS-NON8**: Strict monotonicity - multiple sequential increments produce strictly increasing values with no gaps.

### Overflow Invariants

- **MAGNUS-NON9**: Nonce overflow protection - incrementing a nonce at `u64::MAX` reverts with `NonceOverflow`. Rust uses `checked_add(1)` which returns an error on overflow.
- **MAGNUS-NON10**: Invalid key increment rejection - `increment_nonce(key=0)` reverts with `InvalidNonceKey` (distinct from `ProtocolNonceNotSupported` used for reads).

### Reserved Key Invariants

- **MAGNUS-NON11**: Reserved expiring nonce key - `type(uint256).max` is reserved for `TEMPO_EXPIRING_NONCE_KEY`. Reading it returns 0 for uninitialized accounts (readable but reserved for special use).

## MIP-1015 Compound Policies

MIP-1015 extends MIP-403 with compound policies that specify different authorization rules for senders, recipients, and mint recipients.

### Global Invariants

These are checked after every fuzz run:

- **MAGNUS-1015-2**: Compound policy immutability - compound policies have `PolicyType.COMPOUND` and `admin == address(0)`.
- **MAGNUS-1015-3**: Compound policy existence - all created compound policies return true for `policyExists()`.
- **MAGNUS-1015-4**: Simple policy equivalence - for simple policies, `isAuthorizedSender == isAuthorizedRecipient == isAuthorizedMintRecipient`.
- **MAGNUS-1015-5**: isAuthorized equivalence - for compound policies, `isAuthorized(p, u) == isAuthorizedSender(p, u) && isAuthorizedRecipient(p, u)`.
- **MAGNUS-1015-6**: Compound delegation correctness - directional authorization delegates to the correct sub-policy.

### Per-Handler Assertions

#### Compound Policy Creation

- **MAGNUS-1015-1**: Simple policy constraint - `createCompoundPolicy` reverts with `PolicyNotSimple` if any referenced policy is compound.
- **MAGNUS-1015-2**: Immutability - newly created compound policies have no admin (address(0)).
- **MAGNUS-1015-3**: Existence check - `createCompoundPolicy` reverts with `PolicyNotFound` if any referenced policy doesn't exist.
- **MAGNUS-1015-6**: Built-in policy compatibility - compound policies can reference built-in policies 0 (always-reject) and 1 (always-allow).

#### Compound Policy Modification

- **MAGNUS-1015-2**: Cannot modify compound policy - `modifyPolicyWhitelist` and `modifyPolicyBlacklist` revert for compound policies.

#### MIP-20 Integration

- Mint uses `mintRecipientPolicyId` for authorization (not sender or recipient).
- Transfer uses `senderPolicyId` for sender and `recipientPolicyId` for recipient.
- `burnBlocked` uses `senderPolicyId` to check if address is blocked.
- DEX `cancelStaleOrder` uses `senderPolicyId` to check if maker is blocked.

## MIP20Factory

The MIP20Factory is the factory contract for creating MIP-20 compliant tokens with deterministic addresses.

### Token Creation Invariants

- **MAGNUS-FAC1**: Deterministic addresses - `createToken` deploys to the exact address returned by `getTokenAddress` for the same sender/salt combination.
- **MAGNUS-FAC2**: MIP20 recognition - all tokens created by the factory are recognized as MIP-20 by `isTIP20()`.
- **MAGNUS-FAC3**: Address uniqueness - attempting to create a token at an existing address reverts with `TokenAlreadyExists`.
- **MAGNUS-FAC4**: Quote token validation - `createToken` reverts with `InvalidQuoteToken` if the quote token is not a valid MIP-20.
- **MAGNUS-FAC5**: Reserved address enforcement - addresses in the reserved range (lower 64 bits < 1024) revert with `AddressReserved`.
- **MAGNUS-FAC6**: Token properties - created tokens have correct name, symbol, and currency as specified.
- **MAGNUS-FAC7**: Currency consistency - USD tokens must have USD quote tokens; non-USD tokens can have any valid quote token.

### Address Prediction Invariants

- **MAGNUS-FAC8**: isTIP20 consistency - created tokens return true, non-MIP20 addresses return false.
- **MAGNUS-FAC9**: Address determinism - `getTokenAddress(sender, salt)` always returns the same address for the same inputs.
- **MAGNUS-FAC10**: Sender differentiation - different senders with the same salt produce different token addresses.

### Global Invariants

- **MAGNUS-FAC11**: Address format - all created tokens have addresses with the correct MIP-20 prefix (`0x20C0...`).
- **MAGNUS-FAC12**: Salt-to-token consistency - ghost mappings `saltToToken` and `tokenToSalt` match factory's `getTokenAddress` for all tracked sender/salt combinations.

## MIP403Registry

The MIP403Registry manages transfer policies (whitelists and blacklists) that control which addresses can send or receive tokens.

### Global Invariants

These are checked after every fuzz run:

- **MAGNUS-REG13**: Special policy existence - policies 0 and 1 always exist (return true for `policyExists`).
- **MAGNUS-REG15**: Counter monotonicity - `policyIdCounter` only increases and equals `2 + totalPoliciesCreated`.
- **MAGNUS-REG16**: Policy type immutability - a policy's type cannot change after creation.
- **MAGNUS-REG19**: Policy membership consistency - ghost policy membership state matches registry `isAuthorized` for all tracked accounts, respecting whitelist/blacklist semantics.

### Per-Handler Assertions

These verify correct behavior when the specific function is called:

#### Policy Creation

- **MAGNUS-REG1**: Policy ID assignment - newly created policy ID equals `policyIdCounter` before creation.
- **MAGNUS-REG2**: Counter increment - `policyIdCounter` increments by 1 after each policy creation.
- **MAGNUS-REG3**: Policy existence - all created policies return true for `policyExists()`.
- **MAGNUS-REG4**: Policy data accuracy - `policyData()` returns the correct type and admin as specified during creation.
- **MAGNUS-REG5**: Bulk creation - `createPolicyWithAccounts` correctly initializes all provided accounts in the policy.

#### Admin Management

- **MAGNUS-REG6**: Admin transfer - `setPolicyAdmin` correctly updates the policy admin.
- **MAGNUS-REG7**: Admin-only enforcement - non-admins cannot modify policy admin (reverts with `Unauthorized`).

#### Policy Modification

- **MAGNUS-REG8**: Whitelist modification - adding an account to a whitelist makes `isAuthorized` return true for that account.
- **MAGNUS-REG9**: Blacklist modification - adding an account to a blacklist makes `isAuthorized` return false for that account.
- **MAGNUS-REG10**: Policy type enforcement - `modifyPolicyWhitelist` on a blacklist (or vice versa) reverts with `IncompatiblePolicyType`.

#### Special Policies

- **MAGNUS-REG11**: Always-reject policy - policy ID 0 returns false for all `isAuthorized` checks.
- **MAGNUS-REG12**: Always-allow policy - policy ID 1 returns true for all `isAuthorized` checks.
- **MAGNUS-REG14**: Non-existent policies - policy IDs >= `policyIdCounter` return false for `policyExists()`.
- **MAGNUS-REG17**: Special policy immutability - policies 0 and 1 cannot be modified via `modifyPolicyWhitelist` or `modifyPolicyBlacklist`.
- **MAGNUS-REG18**: Special policy admin immutability - the admin of policies 0 and 1 cannot be changed (attempts revert with `Unauthorized` since admin is `address(0)`).
- **MAGNUS-REG20**: Non-existent policy reverts - `isAuthorized` reverts with `PolicyNotFound` for policy IDs that have never been created.


## ValidatorConfig

The ValidatorConfig precompile manages the set of validators that participate in consensus, including their public keys, addresses, and active status.

### Owner Authorization Invariants

- **MAGNUS-VAL1**: Owner-only add - only the owner can add new validators (non-owners revert with `Unauthorized`).
- **MAGNUS-VAL7**: Owner transfer - `changeOwner` correctly updates the owner address.
- **MAGNUS-VAL8**: New owner authority - only the current owner can transfer ownership.

### Validator Index Invariants

- **MAGNUS-VAL2**: Index assignment - new validators receive sequential indices starting from 0; indices are unique and within bounds.

### Validator Update Invariants

- **MAGNUS-VAL3**: Validator self-update - validators can update their own public key, inbound address, and outbound address.
- **MAGNUS-VAL4**: Update restriction - only the validator themselves can call `updateValidator` (owner cannot update validators).

### Status Management Invariants

- **MAGNUS-VAL5**: Owner-only status change - only the owner can change validator active status (validators cannot change their own status).
- **MAGNUS-VAL6**: Status toggle - `changeValidatorStatus` correctly updates the validator's active flag.

### Validator Creation Invariants

- **MAGNUS-VAL9**: Duplicate rejection - adding a validator that already exists reverts with `ValidatorAlreadyExists`.
- **MAGNUS-VAL10**: Zero public key rejection - adding a validator with zero public key reverts with `InvalidPublicKey`.

### Validator Rotation Invariants

- **MAGNUS-VAL11**: Address rotation - validators can rotate to a new address while preserving their index and active status.

### DKG Ceremony Invariants

- **MAGNUS-VAL12**: DKG epoch setting - `setNextFullDkgCeremony` correctly stores the epoch value.
- **MAGNUS-VAL13**: Owner-only DKG - only the owner can set the DKG ceremony epoch.

### Global Invariants

- **MAGNUS-VAL14**: Owner consistency - contract owner always matches ghost state.
- **MAGNUS-VAL15**: Validator data consistency - all validator data (active status, public key, index) matches ghost state.
- **MAGNUS-VAL16**: Index consistency - each validator's index matches the ghost-tracked index assigned at creation.

## ValidatorConfigV2

The ValidatorConfigV2 precompile replaces V1 with append-only, delete-once semantics. Validators are immutable after creation, tracked by `addedAtHeight` and `deactivatedAtHeight` for historical reconstruction. Ed25519 signature verification proves key ownership at registration. Both owner and validator can call dual-auth functions (rotate, setIpAddresses, transferValidatorOwnership).

### Per-Handler Assertions

- **MAGNUS-VALV2-1**: Dual-auth enforcement - functions callable by owner or validator (`deactivateValidator`, `setIpAddresses`, `rotateValidator`, `transferValidatorOwnership`, `setFeeRecipient`) succeed when called by owner or the validator itself; fail when called by third parties.
- **MAGNUS-VALV2-2**: Owner-only enforcement - functions callable only by owner (`addValidator`, `transferOwnership`, `setNetworkIdentityRotationEpoch`, `migrateValidator`, `initializeIfMigrated`) succeed when called by owner; fail when called by non-owners.
- **MAGNUS-VALV2-3**: Validator count changes - active and total validator counts change only as follows: `addValidator` (+1 active, +1 total), `rotateValidator` (+0 active, +1 total), `deactivateValidator` (-1 active, +0 total); all other operations leave counts unchanged.
- **MAGNUS-VALV2-4**: Height field updates - validator height fields are set only by specific operations and always equal `block.number` when set:
  - `addValidator`: sets new validator's `addedAtHeight = block.number`, `deactivatedAtHeight = 0`
  - `rotateValidator`: sets old validator's `deactivatedAtHeight = block.number`; sets new validator's `addedAtHeight = block.number`, `deactivatedAtHeight = 0`
  - `deactivateValidator`: sets validator's `deactivatedAtHeight = block.number`
  - `migrateValidator`: sets new validator's `addedAtHeight = block.number`, `deactivatedAtHeight = 0` (if V1 active) or `block.number` (if V1 inactive)
- **MAGNUS-VALV2-5**: Init gate enforcement - post-init functions (`addValidator`, `rotateValidator`, `setIpAddresses`, `transferValidatorOwnership`, `setNextDkgCeremony`) fail with `NotInitialized` when `isInitialized() == false`; pre-init functions (`migrateValidator`, `initializeIfMigrated`) fail with `AlreadyInitialized` when `isInitialized() == true`.
- **MAGNUS-VALV2-6**: Address uniqueness per-handler - `transferValidatorOwnership` and `addValidator` rejects addresses already in use by an active validator; `rotateValidator` verifies address mapping points to the new entry after deactivating the old (per-handler supplement to global VALV2-11).
- **MAGNUS-VALV2-7**: Public key validation per-handler - `addValidator` and `rotateValidator` reject zero public keys and public keys already registered (per-handler supplement to global VALV2-12).

### Global Invariants

These are checked after every fuzz run:

- **MAGNUS-VALV2-8**: Append-only - `validatorCount` is monotonically increasing; never decreases across any sequence of operations.
- **MAGNUS-VALV2-9**: Delete-once - no validator can have `deactivatedAtHeight` transition from non-zero back to zero or to a different non-zero value; once deactivated, the validator remains deactivated permanently.
- **MAGNUS-VALV2-10**: Height tracking - for all validators: `addedAtHeight > 0` (set when added and not added during genesis); `deactivatedAtHeight` is either `0` (active) or `>= addedAtHeight` (deactivated at or after addition).
- **MAGNUS-VALV2-11**: Address uniqueness among active - at most one active validator (where `deactivatedAtHeight == 0`) has any given address; deactivated addresses may be reused.
- **MAGNUS-VALV2-12**: Public key uniqueness - all public keys are globally unique, valid, and non-zero across all validators (including deactivated); once registered, a public key cannot be reused.
- **MAGNUS-VALV2-13**: Ingress IP uniqueness - no two active validators share the same ingress IP (port excluded from comparison); deactivated validators' ingress IPs may be reused.
- **MAGNUS-VALV2-14**: Sequential indices - each validator's `index` field equals its position in the validators array (validator at array position `i` has `index == i`).
- **MAGNUS-VALV2-15**: Active validator subset correctness - `getActiveValidators()` returns exactly the set of validators where `deactivatedAtHeight == 0` (no more, no fewer).
- **MAGNUS-VALV2-16**: Validator data consistency - all validator data (publicKey, validatorAddress, ingress, egress, feeRecipient, index, addedAtHeight, deactivatedAtHeight) in contract matches ghost state for each validator.
- **MAGNUS-VALV2-17**: Validator count consistency - `validatorCount()` equals the actual length of the validators array; both are always in sync.
- **MAGNUS-VALV2-18**: `addressToIndex` mapping is accurate - for every validator, `validatorByAddress(validator.validatorAddress)` returns that exact validator.
- **MAGNUS-VALV2-19**: `pubkeyToIndex` mapping is accurate - for every validator, `validatorByPublicKey(validator.publicKey)` returns that exact validator.
- **MAGNUS-VALV2-20**: Owner consistency - `owner()` always equals the ghost-tracked owner; ownership transfers are correctly reflected.
- **MAGNUS-VALV2-21**: Network identity rotation (DKG ceremony) consistency - `getNextNetworkIdentityRotationEpoch()` always equals the ghost-tracked epoch; updates via `setNetworkIdentityRotationEpoch` are correctly stored.
- **MAGNUS-VALV2-22**: Initialization one-way - once `isInitialized() == true`, it remains true forever; `isInitialized()` only transitions from false to true, never back.
- **MAGNUS-VALV2-23**: Migration completeness - if `isInitialized() == false`, then `validatorCount <= V1.getAllValidators().length`; migration cannot exceed V1 validator count.

## AccountKeychain

The AccountKeychain precompile manages authorized Access Keys for accounts, enabling Root Keys to provision scoped secondary keys with expiry timestamps and per-MIP20 token spending limits.

### Global Invariants

These are checked after every fuzz run:

- **MAGNUS-KEY13**: Key data consistency - all key data (expiry, enforceLimits, signatureType) matches ghost state for tracked keys.
- **MAGNUS-KEY14**: Spending limit consistency - all spending limits match ghost state for active keys with limits enforced.
- **MAGNUS-KEY15**: Revocation permanence - revoked keys remain revoked (isRevoked stays true).
- **MAGNUS-KEY16**: Signature type consistency - key signature type matches ghost state for all active keys.

### Per-Handler Assertions

These verify correct behavior when the specific function is called:

#### Key Authorization

- **MAGNUS-KEY1**: Key authorization - `authorizeKey` correctly stores key info (keyId, expiry, signatureType, enforceLimits).
- **MAGNUS-KEY2**: Spending limit initialization - initial spending limits are correctly stored when `enforceLimits` is true.

#### Key Revocation

- **MAGNUS-KEY3**: Key revocation - `revokeKey` marks key as revoked and clears expiry.
- **MAGNUS-KEY4**: Revocation finality - revoked keys cannot be reauthorized (reverts with `KeyAlreadyRevoked`).

#### Spending Limits

- **MAGNUS-KEY5**: Limit update - `updateSpendingLimit` correctly updates the spending limit for a token.
- **MAGNUS-KEY6**: Limit enforcement activation - calling `updateSpendingLimit` on a key with `enforceLimits=false` enables limit enforcement.

#### Input Validation

- **MAGNUS-KEY7**: Zero key rejection - authorizing a key with `keyId=address(0)` reverts with `ZeroPublicKey`.
- **MAGNUS-KEY8**: Duplicate key rejection - authorizing a key that already exists reverts with `KeyAlreadyExists`.
- **MAGNUS-KEY9**: Non-existent key revocation - revoking a key that doesn't exist reverts with `KeyNotFound`.

#### Isolation

- **MAGNUS-KEY10**: Account isolation - keys are scoped per account; the same keyId can be authorized for different accounts with different settings.
- **MAGNUS-KEY11**: Transaction key context - `getTransactionKey` returns `address(0)` when called outside of a transaction signed by an access key.
- **MAGNUS-KEY12**: Non-existent key defaults - `getKey` for a non-existent key returns default values (keyId=0, expiry=0, enforceLimits=false).

#### Expiry Boundaries

- **MAGNUS-KEY17**: Expiry at current timestamp is expired - Rust uses `timestamp >= expiry` so `expiry == block.timestamp` counts as expired.
- **MAGNUS-KEY18**: Operations on expired keys fail with `KeyExpired` - `updateSpendingLimit` on a key where `timestamp >= expiry` reverts.

#### Signature Type Validation

- **MAGNUS-KEY19**: Invalid signature type rejection - enum values >= 3 are invalid and revert with `InvalidSignatureType`.

#### Transaction Context

> **Note**: KEY20/21 cannot be tested in Foundry invariant tests because `transaction_key` uses transient storage (TSTORE/TLOAD) which `vm.store` cannot modify. These invariants require integration tests in `crates/node/tests/it/` that submit real signed transactions.

- **MAGNUS-KEY20**: Main-key-only administration - `authorizeKey`, `revokeKey`, and `updateSpendingLimit` require `transaction_key == 0` (Root Key context). When called with a non-zero transaction key (i.e., from an Access Key), these operations revert with `UnauthorizedCaller`. This ensures only the Root Key can manage Access Keys.
- **MAGNUS-KEY21**: Spending limit tx_origin enforcement - spending limits are only consumed when `msg_sender == tx_origin`. Contract-initiated transfers (where msg_sender is a contract, not the signing EOA) do not consume the EOA's spending limit. This prevents contracts from unexpectedly draining a user's spending limits.

## MIP20

MIP20 is the Magnus token standard that extends ERC-20 with transfer policies, memo support, pause functionality, and reward distribution.

### Transfer Invariants

- **MAGNUS-MIP1**: Balance conservation - sender balance decreases by exactly `amount`, recipient balance increases by exactly `amount`. Transfer returns `true` on success.
- **MAGNUS-MIP2**: Total supply unchanged after transfer - transfers only move tokens between accounts.
- **MAGNUS-MIP3**: Allowance consumption - `transferFrom` decreases allowance by exactly `amount` transferred.
- **MAGNUS-MIP4**: Infinite allowance preserved - `type(uint256).max` allowance remains infinite after `transferFrom`.
- **MAGNUS-MIP9**: Memo transfers behave identically to regular transfers for balance accounting.

### Approval Invariants

- **MAGNUS-MIP5**: Allowance setting - `approve` sets exact allowance amount, returns `true`.
- **MAGNUS-MIP36**: A valid permit sets allowance to the `value` in the permit struct.

### Mint/Burn Invariants

- **MAGNUS-MIP6**: Minting increases total supply and recipient balance by exactly `amount`.
- **MAGNUS-MIP7**: Supply cap enforcement - minting reverts if `totalSupply + amount > supplyCap`.
- **MAGNUS-MIP8**: Burning decreases total supply and burner balance by exactly `amount`.
- **MAGNUS-MIP23**: Burn blocked - `burnBlocked` decreases target balance and total supply by exactly `amount` when target is blacklisted.

### Reward Distribution Invariants

- **MAGNUS-MIP10**: Reward recipient setting - `setRewardRecipient` updates the stored recipient correctly.
- **MAGNUS-MIP11**: Opted-in supply tracking - `optedInSupply` increases when opting in (by holder's balance) and decreases when opting out.
- **MAGNUS-MIP25**: Reward delegation - users can delegate their rewards to another address via `setRewardRecipient`.
- **MAGNUS-MIP12**: Global reward per token updates - `distributeReward` increases `globalRewardPerToken` by `(amount * ACC_PRECISION) / optedInSupply`.
- **MAGNUS-MIP13**: Reward token custody - distributed rewards are transferred to the token contract.
- **MAGNUS-MIP14**: Reward claiming - `claimRewards` transfers owed amount from contract to caller, updates balances correctly.
- **MAGNUS-MIP15**: Claim bounded by available - claimed amount cannot exceed contract's token balance.

### Policy Invariants

- **MAGNUS-MIP16**: Blacklist enforcement - transfers to/from blacklisted addresses revert with `PolicyForbids`.
- **MAGNUS-MIP17**: Pause enforcement - transfers revert with `ContractPaused` when paused.

### Global Invariants

- **MAGNUS-MIP18**: Supply conservation - `totalSupply = initialSupply + totalMints - totalBurns`.
- **MAGNUS-MIP19**: Opted-in supply bounded - `optedInSupply <= totalSupply`.
- **MAGNUS-MIP20**: Balance sum equals supply - sum of all holder balances equals `totalSupply`.
- **MAGNUS-MIP21**: Decimals constant - `decimals()` always returns 6.
- **MAGNUS-MIP22**: Supply cap enforced - `totalSupply <= supplyCap` always holds.

### Protected Address Invariants

- **MAGNUS-MIP24**: Protected address enforcement - `burnBlocked` cannot be called on FeeManager or DEX addresses (reverts with `ProtectedAddress`).

### Access Control Invariants

- **MAGNUS-MIP26**: Issuer-only minting - only accounts with `ISSUER_ROLE` can call `mint` (non-issuers revert with `Unauthorized`).
- **MAGNUS-MIP27**: Pause-role enforcement - only accounts with `PAUSE_ROLE` can call `pause` (non-role holders revert with `Unauthorized`).
- **MAGNUS-MIP28**: Unpause-role enforcement - only accounts with `UNPAUSE_ROLE` can call `unpause` (non-role holders revert with `Unauthorized`).
- **MAGNUS-MIP29**: Burn-blocked-role enforcement - only accounts with `BURN_BLOCKED_ROLE` can call `burnBlocked` (non-role holders revert with `Unauthorized`).

### Permit Invariants

- **MAGNUS-MIP31**: `nonces(owner)` must only ever increase, never decrease.
- **MAGNUS-MIP32**: `nonces(owner)` must increment by exactly 1 on each successful `permit()` call for that owner.
- **MAGNUS-MIP33**: A permit signature can only be used once (enforced by nonce increment).
- **MAGNUS-MIP34**: A permit with a deadline in the past must always revert.
- **MAGNUS-MIP35**: The recovered signer from a valid permit signature must exactly match the `owner` parameter.

## MIP-1020 Signature Verification Precompile

The SignatureVerifier precompile (`0x5165300000000000000000000000000000000000`) verifies Magnus signature types (secp256k1, P256, WebAuthn) onchain via `recover()` and `verify()` functions.

### Differential Verification Invariants

- **SV1**: Transaction-equivalent verification - `recover()` must match `ecrecover` for secp256k1, return the correct P256/WebAuthn-derived address, and `verify()` must return true for correct signers and false for wrong signers. Both raw `v` (0/1) and Ethereum-style `v` (27/28) must be accepted.

### Malleability Resistance Invariants

- **SV2**: P256 and ECDSA signature malleability resistance - signatures with high-s values (`s > n/2`) must be rejected for secp256k1, P256, and WebAuthn (inner P256 signature). Both `recover()` and `verify()` must revert.

### Size Enforcement Invariants

- **SV3**: Signature size enforcement - the precompile must enforce per-type size limits (65 bytes secp256k1, 130 bytes P256, 129–2049 bytes WebAuthn) before any decoding. Wrong-sized inputs and zero-length inputs must revert via both `recover()` and `verify()`.

### Failure Handling Invariants

- **SV4**: Revert on failure - structurally valid but cryptographically invalid (garbage) signatures must cause both `recover()` and `verify()` to revert for all signature types (secp256k1, P256, WebAuthn). Additionally, when `ecrecover` returns `address(0)` for a secp256k1 input, the precompile must revert rather than return a zero address. All reverts must use one of the two defined errors: `InvalidFormat()` (encoding/size issues) or `InvalidSignature()` (cryptographic verification failure).

### Gas Schedule Invariants

- **SV5**: Gas schedule consistency - gas charged must follow the spec (secp256k1: 3,000, P256: 8,000, WebAuthn: 8,000 + input cost). **Not covered in this invariant suite; requires dedicated low-level gas tests.**

### Type Disambiguation Invariants

- **SV6**: Signature type disambiguation - exactly 65 bytes is secp256k1 (no prefix). Non-65-byte signatures with unknown type prefixes must revert via both `recover()` and `verify()`.

### Keychain Rejection Invariants

- **SV7**: Keychain signature rejection - signatures with `0x03` (Keychain secp256k1) or `0x04` (Keychain P256) prefixes must be rejected, even when containing valid-looking inner signatures. Both `recover()` and `verify()` must revert. The precompile may return either `InvalidFormat()` (when the keychain prefix is rejected at the parsing layer as an unsupported type) or `InvalidSignature()` (if parsing succeeds but verification rejects it).

## MIP-1022 Virtual Addresses

### Registry & Address Invariants

- **MAGNUS-VA1**: Registration determinism - each fixed `(master, salt)` fixture registers exactly the `masterId` implied by `bytes4(keccak256(abi.encodePacked(master, salt))[4:8])`.
- **MAGNUS-VA2**: Master ID uniqueness - no two registered fixtures share a `masterId`.
- **MAGNUS-VA3**: Decode round-trip - `decodeVirtualAddress(makeVirtualAddress(masterId, userTag))` returns the original `masterId` and `userTag`.
- **MAGNUS-VA4**: Registered resolution - `resolveRecipient(virtual)` and `resolveVirtualAddress(virtual)` both return the registered master for every tracked alias.
- **MAGNUS-VA5**: Non-virtual passthrough - `resolveRecipient(nonVirtual)` returns the literal address unchanged.

### MIP-20 Forwarding Invariants

- **MAGNUS-VA6**: Unregistered resolution is atomic - calls to unregistered virtual aliases revert with no balance, allowance, supply, or event changes.
- **MAGNUS-VA7**: Transfer forwarding exactness - `transfer` and `transferWithMemo` debit the sender exactly once and credit the resolved master exactly once.
- **MAGNUS-VA8**: Allowance forwarding exactness - `transferFrom` and `transferFromWithMemo` apply forwarding without changing allowance semantics.
- **MAGNUS-VA9**: Mint forwarding exactness - `mint` and `mintWithMemo` credit only the resolved master and increase total supply by exactly `amount`.
- **MAGNUS-VA10**: Zero-balance invariant - `balanceOf(virtual) == 0` for every tracked alias after every run.
- **MAGNUS-VA11**: Two-hop transfer events - plain transfer paths emit `Transfer(sender, virtual, amount)` followed by `Transfer(virtual, master, amount)`.
- **MAGNUS-VA12**: Memo and mint event attribution - memo events and `Mint` events use the virtual alias as the recipient-facing address, with the forwarding hop emitted last.
- **MAGNUS-VA13**: Self-forward neutrality - master-to-own-alias transfers have zero net balance effect on the master while still emitting both hops.

### Policy & Reward Invariants

- **MAGNUS-VA14**: Policy-on-master semantics - recipient and mint-recipient authorization is evaluated on the resolved master, not the alias.
- **MAGNUS-VA15**: Policy-operation rejection - MIP-403 configuration APIs reject virtual aliases as literal policy members.
- **MAGNUS-VA16**: Reward-recipient rejection - `setRewardRecipient` rejects virtual aliases.
