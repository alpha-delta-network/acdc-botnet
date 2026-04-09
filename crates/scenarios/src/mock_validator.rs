// G08: In-process BFT mock validator harness.
// Provides lightweight MockValidator and BftRound types for deterministic Byzantine tests.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// The mode a validator operates in during a round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByzantineMode {
    /// Votes honestly once per round.
    Honest,
    /// Sends two conflicting votes for the same round (equivocation).
    EquivocatingVote,
    /// Drops all messages silently (no votes).
    SilentDropper,
    /// Proposes two different blocks as leader.
    DoubleProposer,
}

/// A vote cast by a validator in a BFT round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BftVote {
    /// ID of the voting validator.
    pub validator_id: usize,
    /// Round number this vote is for.
    pub round: u64,
    /// Block hash voted for (0 = block A, 1 = block B for equivocators).
    pub block_hash: u8,
}

/// A block proposal from a leader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProposal {
    pub proposer_id: usize,
    pub round: u64,
    pub block_hash: u8,
}

/// Messages passed on the BFT channel.
#[derive(Debug, Clone)]
pub enum BftMessage {
    /// A vote for a block in a round.
    Vote(BftVote),
    /// A block proposal from a leader.
    Proposal(BlockProposal),
}

/// Result of attempting to collect votes for a round.
#[derive(Debug, PartialEq, Eq)]
pub enum VoteResult {
    /// Quorum reached: these validator IDs signed.
    Quorum(Vec<usize>),
    /// Not enough votes before the step limit.
    Timeout,
    /// Conflicting votes from equivocators prevented a clean quorum.
    NoQuorum,
}

/// Equivocation evidence: a validator sent two different votes for the same round.
#[derive(Debug, Clone)]
pub struct EquivocationEvidence {
    pub validator_id: usize,
    pub round: u64,
    pub vote_a: BftVote,
    pub vote_b: BftVote,
}

/// Lightweight in-process validator for BFT testing.
pub struct MockValidator {
    pub validator_id: usize,
    pub is_byzantine: bool,
    pub mode: ByzantineMode,
    /// The last round this validator has voted in (monotonically increases for honest nodes).
    pub signed_round: Arc<AtomicU64>,
    pub message_tx: mpsc::UnboundedSender<BftMessage>,
    pub message_rx: mpsc::UnboundedReceiver<BftMessage>,
}

impl MockValidator {
    /// Create a new honest validator.
    pub fn honest(validator_id: usize) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            validator_id,
            is_byzantine: false,
            mode: ByzantineMode::Honest,
            signed_round: Arc::new(AtomicU64::new(0)),
            message_tx: tx,
            message_rx: rx,
        }
    }

    /// Create a new Byzantine validator with the specified mode.
    pub fn byzantine(validator_id: usize, mode: ByzantineMode) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            validator_id,
            is_byzantine: true,
            mode,
            signed_round: Arc::new(AtomicU64::new(0)),
            message_tx: tx,
            message_rx: rx,
        }
    }

    /// Produce votes for the given round.
    /// Honest: one vote. Equivocating: two different votes. SilentDropper: none.
    /// DoubleProposer behaves like honest for voting (cheating is in proposals).
    pub fn cast_votes(&self, round: u64) -> Vec<BftVote> {
        match &self.mode {
            ByzantineMode::Honest | ByzantineMode::DoubleProposer => {
                self.signed_round.store(round, Ordering::SeqCst);
                vec![BftVote {
                    validator_id: self.validator_id,
                    round,
                    block_hash: 0,
                }]
            }
            ByzantineMode::EquivocatingVote => {
                // Two conflicting votes for the same round — equivocation
                self.signed_round.store(round, Ordering::SeqCst);
                vec![
                    BftVote {
                        validator_id: self.validator_id,
                        round,
                        block_hash: 0,
                    },
                    BftVote {
                        validator_id: self.validator_id,
                        round,
                        block_hash: 1, // conflicting block
                    },
                ]
            }
            ByzantineMode::SilentDropper => {
                // Cast no votes
                vec![]
            }
        }
    }

    /// Produce block proposals for the given round (used when this validator is leader).
    pub fn propose_blocks(&self, round: u64) -> Vec<BlockProposal> {
        match &self.mode {
            ByzantineMode::DoubleProposer => vec![
                BlockProposal {
                    proposer_id: self.validator_id,
                    round,
                    block_hash: 0,
                },
                BlockProposal {
                    proposer_id: self.validator_id,
                    round,
                    block_hash: 1,
                },
            ],
            _ => vec![BlockProposal {
                proposer_id: self.validator_id,
                round,
                block_hash: 0,
            }],
        }
    }
}

/// Coordinates a single BFT round among N validators.
pub struct BftRound {
    pub round: u64,
    pub n_validators: usize,
    pub quorum_size: usize,
}

impl BftRound {
    /// Create a new round coordinator.
    /// `quorum_size` is the minimum number of non-conflicting votes required.
    pub fn new(round: u64, n_validators: usize, quorum_size: usize) -> Self {
        Self {
            round,
            n_validators,
            quorum_size,
        }
    }

    /// Collect votes from all validators and determine the outcome.
    ///
    /// The algorithm:
    /// 1. Gather all votes from every validator.
    /// 2. Detect equivocators (same validator_id, same round, different block_hash).
    /// 3. Discard all votes from detected equivocators.
    /// 4. Among remaining honest votes, check if any block_hash has >= quorum_size votes.
    ///
    /// This is deterministic — no real timeouts needed.
    pub fn collect_votes(
        &self,
        validators: &[MockValidator],
    ) -> (VoteResult, Vec<EquivocationEvidence>) {
        let mut all_votes: Vec<BftVote> = Vec::new();

        for v in validators {
            let votes = v.cast_votes(self.round);
            all_votes.extend(votes);
        }

        // Group votes by validator_id
        let mut by_validator: HashMap<usize, Vec<BftVote>> = HashMap::new();
        for vote in &all_votes {
            by_validator
                .entry(vote.validator_id)
                .or_default()
                .push(vote.clone());
        }

        // Detect equivocators: same validator, same round, different block_hash values
        let mut equivocations: Vec<EquivocationEvidence> = Vec::new();
        let mut equivocator_ids: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        for (vid, votes) in &by_validator {
            if votes.len() > 1 {
                // Check for different block hashes
                let first_hash = votes[0].block_hash;
                let has_conflict = votes.iter().any(|v| v.block_hash != first_hash);
                if has_conflict {
                    equivocator_ids.insert(*vid);
                    equivocations.push(EquivocationEvidence {
                        validator_id: *vid,
                        round: self.round,
                        vote_a: votes[0].clone(),
                        vote_b: votes[1].clone(),
                    });
                }
            }
        }

        // Count valid votes (exclude equivocators), grouped by block_hash
        let mut hash_votes: HashMap<u8, Vec<usize>> = HashMap::new();
        for vote in &all_votes {
            if !equivocator_ids.contains(&vote.validator_id) {
                hash_votes
                    .entry(vote.block_hash)
                    .or_default()
                    .push(vote.validator_id);
            }
        }

        // Check if any block_hash reached quorum
        for (_, signers) in &hash_votes {
            // Deduplicate signer IDs (honest validators only vote once)
            let mut unique_signers = signers.clone();
            unique_signers.sort_unstable();
            unique_signers.dedup();

            if unique_signers.len() >= self.quorum_size {
                return (VoteResult::Quorum(unique_signers), equivocations);
            }
        }

        // No quorum — if we had equivocators, it's NoQuorum; if just silence, Timeout
        if !equivocations.is_empty() {
            (VoteResult::NoQuorum, equivocations)
        } else {
            (VoteResult::Timeout, equivocations)
        }
    }

    /// Check if a set of proposals from a leader contains conflicting proposals (DoubleProposer).
    pub fn check_double_proposals(proposals: &[BlockProposal]) -> bool {
        if proposals.len() < 2 {
            return false;
        }
        let first_hash = proposals[0].block_hash;
        proposals.iter().any(|p| p.block_hash != first_hash)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn honest_validator_casts_one_vote() {
        let v = MockValidator::honest(0);
        let votes = v.cast_votes(1);
        assert_eq!(votes.len(), 1);
        assert_eq!(votes[0].block_hash, 0);
    }

    #[test]
    fn equivocating_validator_casts_two_conflicting_votes() {
        let v = MockValidator::byzantine(1, ByzantineMode::EquivocatingVote);
        let votes = v.cast_votes(1);
        assert_eq!(votes.len(), 2);
        assert_ne!(votes[0].block_hash, votes[1].block_hash);
    }

    #[test]
    fn silent_dropper_casts_no_votes() {
        let v = MockValidator::byzantine(2, ByzantineMode::SilentDropper);
        assert!(v.cast_votes(1).is_empty());
    }

    #[test]
    fn double_proposer_proposes_two_blocks() {
        let v = MockValidator::byzantine(3, ByzantineMode::DoubleProposer);
        let proposals = v.propose_blocks(1);
        assert_eq!(proposals.len(), 2);
        assert!(BftRound::check_double_proposals(&proposals));
    }
}
