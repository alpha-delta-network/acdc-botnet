# Developer Anti-Patterns Research

**Status**: Phase 3 - Common developer mistakes that should fail gracefully
**Priority**: P0 (must handle) to P3 (nice-to-have)

## Category 1: Parameter Validation Errors (P0)

### PT-D-001: Invalid Signature (P0)
**Description**: Submit transaction with malformed or invalid signature
**Common mistake**: Developer forgets to sign, signs with wrong key, corrupts signature
**Expected behavior**: Transaction rejected with clear error message
**Test**: Submit transaction with zero signature, wrong length, invalid curve point

### PT-D-002: Invalid Format (P0)
**Description**: Transaction doesn't match expected format
**Common mistake**: Missing required fields, wrong field types, incorrect encoding
**Expected behavior**: Parsing error with specific field identified
**Test**: Missing `nonce`, wrong `amount` type (string vs number), malformed JSON

### PT-D-003: Missing Required Fields (P0)
**Description**: Transaction lacks mandatory fields
**Common mistake**: Developer uses old API version, copies incomplete example
**Expected behavior**: Validation error listing missing fields
**Test**: Omit `to` address, omit `amount`, omit `chain_id`

## Category 2: State Assumptions (P0-P1)

### PT-D-010: Insufficient Balance (P0)
**Description**: Attempt to spend more than available balance
**Common mistake**: Developer doesn't check balance first, race condition with another tx
**Expected behavior**: "Insufficient funds" error
**Test**: Try to transfer 1000 AX when balance is 500 AX

### PT-D-011: Double-Spend Attempt (P0)
**Description**: Submit two transactions spending same funds
**Common mistake**: Concurrent operations, nonce management errors
**Expected behavior**: Second transaction rejected with nonce error
**Test**: Submit two transactions with same nonce

### PT-D-012: Stale Nonce (P0)
**Description**: Use outdated nonce value
**Common mistake**: Cached nonce, parallel transaction submission
**Expected behavior**: "Invalid nonce" error with expected value
**Test**: Submit transaction with nonce=5 when current nonce=10

### PT-D-013: Voting After Deadline (P1)
**Description**: Attempt to vote after voting period ended
**Common mistake**: Developer doesn't check timing, relies on client-side validation only
**Expected behavior**: "Voting period ended" error
**Test**: Submit vote after `voting_end_block` passed

## Category 3: Timing/Ordering Errors (P0-P1)

### PT-D-020: Pre-Timelock Execution (P0)
**Description**: Try to execute proposal before timelock expires
**Common mistake**: Developer bypasses timelock check, miscalculates block timing
**Expected behavior**: "Timelock not expired" error
**Test**: Execute proposal immediately after passing vote

### PT-D-021: Late Vote (P1)
**Description**: Vote after voting window closed
**Common mistake**: Network delay, incorrect timing calculation
**Expected behavior**: "Voting period closed" error
**Test**: Vote at `voting_end_block + 1`

### PT-D-022: Expired Proof (P1)
**Description**: Submit ZK proof after expiration time
**Common mistake**: Slow proof generation, network delay
**Expected behavior**: "Proof expired" error with validity window
**Test**: Generate proof, wait past expiration, submit

## Category 4: Type Confusion (P0-P1)

### PT-D-030: Wrong Chain Transaction (P0)
**Description**: Submit Alpha transaction to Delta node
**Common mistake**: Wrong RPC endpoint, incorrect chain ID
**Expected behavior**: "Invalid chain ID" error
**Test**: Send Alpha-formatted tx to Delta endpoint

### PT-D-031: Wrong Network (Testnet vs Mainnet) (P0)
**Description**: Submit testnet transaction to mainnet
**Common mistake**: Environment variable not updated, wrong config
**Expected behavior**: "Network mismatch" error
**Test**: Send testnet tx (network=13) to mainnet (network=8)

### PT-D-032: Wrong Token Type (P1)
**Description**: Try to transfer sAX on Alpha (should be Delta only)
**Common mistake**: Confusion between AX and sAX
**Expected behavior**: "Invalid token for this chain" error
**Test**: Call `transfer sAX` on Alpha endpoint

## Category 5: Missing Prerequisites (P0-P1)

### PT-D-040: Unstaked Voting (P0)
**Description**: Attempt to vote without staking required tokens
**Common mistake**: Developer assumes all addresses can vote
**Expected behavior**: "Insufficient stake for voting" error
**Test**: Vote with address that has 0 stake

### PT-D-041: Unregistered Governor (P0)
**Description**: Non-governor tries to create proposal
**Common mistake**: Developer doesn't check governor status
**Expected behavior**: "Not a registered governor" error
**Test**: Create proposal from non-governor address

### PT-D-042: Missing Prior Lock (P1)
**Description**: Try to unlock AX without prior lock
**Common mistake**: Developer submits unlock without lock transaction
**Expected behavior**: "No corresponding lock found" error
**Test**: Submit unlock with random unlock_id

### PT-D-043: Unregistered Validator (P1)
**Description**: Non-validator tries to propose block
**Common mistake**: Developer thinks any node can propose
**Expected behavior**: "Not in validator set" error
**Test**: Attempt block proposal from non-validator

## Category 6: Boundary Conditions (P1-P2)

### PT-D-050: Integer Overflow (P1)
**Description**: Amount exceeds maximum value
**Common mistake**: Developer uses wrong data type (u32 instead of u128)
**Expected behavior**: "Amount exceeds maximum" error
**Test**: Transfer 2^128 tokens (max u128 + 1)

### PT-D-051: Zero Amount (P1)
**Description**: Transfer/trade with amount=0
**Common mistake**: Developer allows 0 in UI, doesn't validate
**Expected behavior**: "Amount must be positive" error
**Test**: Transfer 0 AX

### PT-D-052: Maximum Size Exceeded (P2)
**Description**: Transaction/program exceeds size limit
**Common mistake**: Developer doesn't check size constraints
**Expected behavior**: "Transaction too large" error
**Test**: Submit 10MB transaction (max is 1MB)

### PT-D-053: Negative Amount (P1)
**Description**: Use negative number for amount (if API allows)
**Common mistake**: Incorrect type casting, missing validation
**Expected behavior**: "Amount must be non-negative" error
**Test**: Transfer -100 AX (if type system allows)

## Category 7: Async/Concurrency Errors (P1-P2)

### PT-D-060: Orphan Transaction (P1)
**Description**: Transaction depends on parent that didn't confirm
**Common mistake**: Developer assumes synchronous confirmation
**Expected behavior**: Transaction waits or fails gracefully
**Test**: Submit child tx immediately after parent (before confirmation)

### PT-D-061: Reorg Conflict (P2)
**Description**: Transaction confirmed, then reverted by reorg
**Common mistake**: Developer assumes finality after 1 block
**Expected behavior**: Transaction re-enters mempool or explicit failure
**Test**: Cause deliberate reorg after tx confirmation

### PT-D-062: Race Condition (P1)
**Description**: Two operations on same resource simultaneously
**Common mistake**: No locking, optimistic concurrency not handled
**Expected behavior**: One succeeds, other fails with clear error
**Test**: Two bots cancel same order simultaneously

## Category 8: Configuration Errors (P1-P2)

### PT-D-070: Wrong Endpoint (P1)
**Description**: Connect to wrong RPC endpoint
**Common mistake**: Hardcoded localhost, incorrect port
**Expected behavior**: Connection error or "Unknown method" error
**Test**: Call Alpha method on Delta endpoint

### PT-D-071: Unsupported Feature (P2)
**Description**: Use feature not enabled on this network
**Common mistake**: Call mainnet-only feature on testnet
**Expected behavior**: "Feature not available" error
**Test**: Try to use feature with required flag disabled

### PT-D-072: Deprecated API (P2)
**Description**: Call deprecated API endpoint
**Common mistake**: Use old SDK, outdated documentation
**Expected behavior**: Deprecation warning + error
**Test**: Call /v1/endpoint when /v2 is required

## Implementation Priority

| Pattern | Priority | Frequency | Severity | User Impact |
|---------|----------|-----------|----------|-------------|
| PT-D-001 | P0 | Very High | High | Transaction fails |
| PT-D-010 | P0 | High | High | Transaction fails |
| PT-D-030 | P0 | Medium | Critical | Wrong chain |
| PT-D-040 | P0 | Medium | High | Auth failure |
| PT-D-020 | P0 | Medium | High | Timing error |

**Total P0 patterns**: 15
**Total P1 patterns**: 12
**Total P2-P3 patterns**: 8

## Testing Strategy

1. **Error Message Quality**: Verify all errors are clear and actionable
2. **Graceful Degradation**: System remains stable after error
3. **No Silent Failures**: Every error is reported
4. **Consistent Format**: All error messages follow same structure

## Expected Error Format

```json
{
  "error": {
    "code": "INSUFFICIENT_BALANCE",
    "message": "Insufficient balance: available 500 AX, required 1000 AX",
    "field": "amount",
    "suggestion": "Check balance before transfer"
  }
}
```

## Next Steps

1. Implement all P0 anti-pattern behaviors
2. Verify error messages are developer-friendly
3. Test error recovery paths
4. Document common mistakes in user guide
