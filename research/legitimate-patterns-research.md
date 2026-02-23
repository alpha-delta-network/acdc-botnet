# Legitimate User Patterns Research

**Status**: Phase 2 - Research queries for Gemini execution
**Priority**: P0-P3 patterns across 6 categories

## Research Queries for Gemini

### Query 1: Blockchain Governance Voting Patterns
**Search**: "blockchain governance voting patterns production deployment real-world"
**Focus**:
- Typical voting participation rates
- Proposal lifecycle (draft → vote → timelock → execute)
- Quorum requirements
- Delegation patterns
- Abstention vs active voting

**Expected patterns**:
- PT-L-001: Basic proposal voting (create → vote → result)
- PT-L-002: Delegated voting
- PT-L-003: Time-locked governance execution
- PT-L-004: Governance parameter updates

### Query 2: Cross-Chain Bridge Testing Methodology
**Search**: "cross-chain bridge testing patterns atomic swaps lock mint methodology"
**Focus**:
- Lock/mint workflows
- Burn/unlock patterns
- Atomic swap mechanics
- Finality requirements
- Timeout handling

**Expected patterns**:
- PT-L-010: Basic lock→mint→burn→unlock flow
- PT-L-011: Multi-step cross-chain coordination
- PT-L-012: Cross-chain atomicity verification
- PT-L-013: Finality waiting patterns

### Query 3: DEX Integration Test Scenarios
**Search**: "DEX integration testing orderbook matching AMM liquidity provider patterns"
**Focus**:
- Order placement and matching
- Market vs limit orders
- Partial fills
- Order cancellation
- Liquidity provision

**Expected patterns**:
- PT-L-020: Spot market order execution
- PT-L-021: Limit order placement and fill
- PT-L-022: Order cancellation
- PT-L-023: Liquidity pool operations
- PT-L-024: Slippage handling

### Query 4: zkSNARK Testing Patterns
**Search**: "zkSNARK testing patterns privacy blockchain shielded transactions proof generation"
**Focus**:
- Proof generation workflows
- Verification patterns
- Privacy-preserving transfers
- Address recycling
- Mixing strategies

**Expected patterns**:
- PT-L-030: Shielded transfer (generate proof → submit)
- PT-L-031: Address recycling for privacy
- PT-L-032: Batch proof generation
- PT-L-033: Privacy set mixing

### Query 5: BFT Consensus Testing
**Search**: "BFT consensus testing patterns validator participation block attestation"
**Focus**:
- Block proposal patterns
- Attestation signing
- Validator rotation
- Reward claiming
- Slashing conditions

**Expected patterns**:
- PT-L-040: Validator block proposal
- PT-L-041: Block attestation
- PT-L-042: Reward claiming
- PT-L-043: Validator registration/deregistration

### Query 6: Perpetuals Trading Patterns
**Search**: "perpetual futures trading patterns leverage liquidation funding rate"
**Focus**:
- Position opening (long/short)
- Leverage management
- Position closing
- Liquidation triggers
- Funding rate payments

**Expected patterns**:
- PT-L-050: Open perpetual position
- PT-L-051: Close position (profit/loss)
- PT-L-052: Liquidation avoidance
- PT-L-053: Funding rate optimization

## Documented Patterns (Based on Best Practices)

### Category 1: Governance Workflows (P0-P1)

#### PT-L-001: Basic Proposal Lifecycle (P0)
**Description**: Complete governance proposal flow from creation to execution
**Steps**:
1. Create proposal with valid parameters
2. Wait for voting period start
3. Cast vote (yes/no/abstain)
4. Wait for voting period end
5. Wait for timelock
6. Execute proposal

**Test coverage**:
- Valid proposal formats
- Timelock enforcement
- Vote counting
- Execution success

#### PT-L-002: Delegated Voting (P1)
**Description**: Delegate voting power to another address
**Steps**:
1. Delegate tokens to representative
2. Representative votes on behalf
3. Verify vote weight includes delegated power
4. Undelegate after proposal

#### PT-L-003: Joint Alpha/Delta Governance (P0)
**Description**: Proposals that affect both chains
**Steps**:
1. Create proposal on Alpha
2. Proposal automatically mirrored to Delta
3. Vote on both chains
4. Execution requires quorum on both
5. State changes propagate via IPC

### Category 2: Cross-Chain Coordination (P0)

#### PT-L-010: Lock/Mint/Burn/Unlock Flow (P0)
**Description**: Full cross-chain asset transfer cycle
**Steps**:
1. Lock AX on Alpha (generate unlock_id)
2. Wait for Alpha finality (3 blocks)
3. IPC message to Delta
4. Mint sAX on Delta (same amount)
5. Verify sAX balance
6. [Later] Burn sAX on Delta
7. IPC message to Alpha
8. Unlock AX on Alpha

**Success criteria**:
- Atomicity: no double-spend
- Conservation: total supply unchanged
- Finality: irreversible after confirmed

#### PT-L-011: Concurrent Cross-Chain Operations (P1)
**Description**: Multiple users performing cross-chain transfers simultaneously
**Steps**:
1. 100 bots lock AX on Alpha (different amounts)
2. Verify all IPC messages sent
3. Verify all sAX minted on Delta
4. Check total locked = total minted

### Category 3: Trading Lifecycle (P0-P1)

#### PT-L-020: Spot Market Order (P0)
**Description**: Basic market order execution
**Steps**:
1. Query orderbook for pair (e.g., AX/DX)
2. Place market buy order
3. Verify immediate fill (or best available)
4. Check balance updated
5. Verify trade appears in history

#### PT-L-021: Limit Order Lifecycle (P0)
**Description**: Place, partial fill, cancel flow
**Steps**:
1. Place limit buy order below market price
2. Wait for partial fill (50%)
3. Verify balance updated for filled portion
4. Cancel remaining order
5. Verify order removed from book

#### PT-L-022: Liquidity Provider Operations (P1)
**Description**: Add/remove liquidity to AMM pool
**Steps**:
1. Approve token spending
2. Add liquidity (AX + DX)
3. Receive LP tokens
4. Wait for trading fees to accumulate
5. Remove liquidity
6. Verify fee rewards received

### Category 4: Privacy Operations (P1-P2)

#### PT-L-030: Shielded Transfer (P1)
**Description**: Private AlphaVM transfer using ZK proofs
**Steps**:
1. Generate proof for transfer (amount, recipient)
2. Submit shielded transaction
3. Wait for proof verification (can be slow)
4. Verify transaction confirmed
5. Recipient verifies balance (off-chain)

**Constraints**:
- Proof generation: 5-10 seconds
- Verification: <1 second on-chain

#### PT-L-031: Address Recycling (P2)
**Description**: Rotate receiving addresses for privacy
**Steps**:
1. Generate new stealth address
2. Publish address to recipient (off-chain)
3. Receive transfer to new address
4. Never reuse address

### Category 5: Validator Participation (P0-P1)

#### PT-L-040: Validator Block Proposal (P0)
**Description**: Propose a block as validator
**Steps**:
1. Verify validator is in active set
2. Wait for turn (round-robin or random)
3. Collect transactions from mempool
4. Build block with valid header
5. Propose block to network
6. Collect attestations (>2/3 stake)
7. Finalize block

#### PT-L-041: Continuous Block Attestation (P0)
**Description**: Attest to blocks proposed by others
**Steps**:
1. Monitor for new block proposals
2. Verify block validity
3. Sign attestation
4. Broadcast attestation
5. Track attestation rewards

#### PT-L-042: Rewards Claiming (P1)
**Description**: Claim accumulated validator rewards
**Steps**:
1. Query pending rewards
2. Submit claim transaction
3. Verify rewards credited to balance
4. Optional: re-stake rewards

### Category 6: Time-Dependent Operations (P1-P2)

#### PT-L-060: Auction Participation (P2)
**Description**: Vickrey auction for name service
**Steps**:
1. Commit phase: Submit hash(bid, salt)
2. Wait for commit period end
3. Reveal phase: Submit (bid, salt)
4. Wait for reveal period end
5. Auction resolution: highest bidder wins
6. Refund losing bids

## Alpha/Delta-Specific Mappings

### Alpha Chain
- Governance: Native GID tokens, 7-day timelock
- Privacy: AlphaVM shielded transfers (Groth16)
- Validator: BFT consensus, 80 validators
- Cross-chain: Lock AX → emit IPC event

### Delta Chain
- DEX: Orderbook + AMM hybrid
- Perpetuals: Up to 20x leverage
- Oracles: Median of 7 price feeds
- Cross-chain: Mint sAX ← receive IPC event

## Priority Matrix

| Pattern | Priority | Complexity | Alpha/Delta | Test Duration |
|---------|----------|------------|-------------|---------------|
| PT-L-001 | P0 | Medium | Both | 2-3 min |
| PT-L-010 | P0 | High | Both | 1-2 min |
| PT-L-020 | P0 | Low | Delta | 10-30 sec |
| PT-L-021 | P0 | Medium | Delta | 30-60 sec |
| PT-L-030 | P1 | High | Alpha | 10-15 sec |
| PT-L-040 | P0 | High | Both | 1-2 min |

## Implementation Notes

1. **All P0 patterns must be implemented in Phase 2**
2. **P1 patterns should be implemented if time permits**
3. **P2-P3 patterns are optional enhancements**
4. **Each pattern needs**:
   - Behavior trait implementation
   - Pre/post-condition checks
   - Unit scenario YAML
   - Integration test

## Next Steps

1. Execute Gemini research queries to validate/expand patterns
2. Implement behaviors for all P0 patterns (Tasks #10-11)
3. Create unit scenarios for each pattern
4. Verify behaviors with integration tests
