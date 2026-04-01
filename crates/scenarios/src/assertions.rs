/// MECE Assertion Registry — every UC appears exactly once, every action_type maps to exactly
/// one UC. Validated at test startup via `validate_mece()`.
///
/// MECE = Mutually Exclusive, Collectively Exhaustive.
/// - Mutually Exclusive: no uc_id appears twice; no action_type appears twice.
/// - Collectively Exhaustive: all 13 UCs in the sprint 4+5 spec are present.

/// A single registered assertion entry.
#[derive(Debug, Clone)]
pub struct AssertionEntry {
    /// UC identifier, e.g. "UC-GOV-001"
    pub uc_id: &'static str,
    /// Bot action type string that triggers this UC, e.g. "governance_vote"
    pub action_type: &'static str,
    /// Human-readable description of what is being verified
    pub description: &'static str,
}

/// Registry holding the canonical MECE assertion map.
pub struct AssertionRegistry {
    entries: Vec<AssertionEntry>,
}

impl AssertionRegistry {
    /// Build the canonical registry. Every UC appears exactly once.
    ///
    /// Contains all 13 UCs: UC-GOV-001..004, UC-GID-001..004, UC-DEX-001..003,
    /// UC-BAL-001, UC-TXN-001.
    pub fn canonical() -> Self {
        let entries = vec![
            // ── Governance UCs ──────────────────────────────────────────────
            AssertionEntry {
                uc_id: "UC-GOV-001",
                action_type: "governance_vote",
                description: "Voter public key appears in proposal votes after casting a vote",
            },
            AssertionEntry {
                uc_id: "UC-GOV-002",
                action_type: "governance_proposal_create",
                description: "Newly created proposal appears in the governance proposals list",
            },
            AssertionEntry {
                uc_id: "UC-GOV-003",
                action_type: "governance_finalize",
                description: "Proposal status transitions to 'passed' or 'failed' after finalize",
            },
            AssertionEntry {
                uc_id: "UC-GOV-004",
                action_type: "governance_execute",
                description: "Proposal status is 'executed' after the execute call",
            },
            // ── GID UCs ─────────────────────────────────────────────────────
            AssertionEntry {
                uc_id: "UC-GID-001",
                action_type: "gid_register",
                description: "GID address is active (HTTP 200) in /api/governance/gci/{gid_address}",
            },
            AssertionEntry {
                uc_id: "UC-GID-002",
                action_type: "grim_trigger_check",
                description: "Grim trigger response for GID address contains 'crippled' field",
            },
            AssertionEntry {
                uc_id: "UC-GID-003",
                action_type: "apology_lifecycle",
                description: "Proposal with proposal_type containing 'apology' exists after apology submission",
            },
            AssertionEntry {
                uc_id: "UC-GID-004",
                action_type: "multisig_vote",
                description: "Proposal votes list contains multiple voter_public_keys",
            },
            // ── DEX UCs ─────────────────────────────────────────────────────
            AssertionEntry {
                uc_id: "UC-DEX-001",
                action_type: "dex_order_place",
                description: "Orderbook for market contains order or tx_id matching the placed order",
            },
            AssertionEntry {
                uc_id: "UC-DEX-002",
                action_type: "dex_limit_order",
                description: "Orderbook for market shows limit order presence on the correct side",
            },
            AssertionEntry {
                uc_id: "UC-DEX-003",
                action_type: "dex_order_cancel",
                description: "Orderbook for market no longer contains the cancelled order tx_id",
            },
            // ── Balance UCs ──────────────────────────────────────────────────
            AssertionEntry {
                uc_id: "UC-BAL-001",
                action_type: "balance_query",
                description: "GET /api/v1/addresses/{address}/balance returns HTTP 200 with balance field",
            },
            // ── Transaction UCs ──────────────────────────────────────────────
            AssertionEntry {
                uc_id: "UC-TXN-001",
                action_type: "private_transfer",
                description: "Transaction exists via tx lookup or recipient balance delta is positive",
            },
        ];

        Self { entries }
    }

    /// Panic if any uc_id is duplicated or if any action_type maps to more than one UC.
    ///
    /// Call this at test startup (e.g. in a `#[test]` or at the top of `run_gauntlet_light`).
    pub fn validate_mece(&self) {
        use std::collections::HashMap;

        let mut uc_ids: HashMap<&'static str, usize> = HashMap::new();
        let mut action_types: HashMap<&'static str, usize> = HashMap::new();

        for (idx, entry) in self.entries.iter().enumerate() {
            if let Some(prev_idx) = uc_ids.insert(entry.uc_id, idx) {
                panic!(
                    "MECE violation: uc_id '{}' appears at index {} and again at index {}",
                    entry.uc_id, prev_idx, idx
                );
            }
            if let Some(prev_idx) = action_types.insert(entry.action_type, idx) {
                panic!(
                    "MECE violation: action_type '{}' appears at index {} and again at index {} (uc_ids: '{}' and '{}')",
                    entry.action_type,
                    prev_idx,
                    idx,
                    self.entries[prev_idx].uc_id,
                    entry.uc_id,
                );
            }
        }
    }

    /// Return the UC ID for a given action_type, or None if unregistered.
    pub fn uc_for_action(&self, action_type: &str) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|e| e.action_type == action_type)
            .map(|e| e.uc_id)
    }

    /// Return all entries in the registry (read-only).
    pub fn entries(&self) -> &[AssertionEntry] {
        &self.entries
    }

    /// Return the total number of registered UCs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_registry_has_13_entries() {
        let registry = AssertionRegistry::canonical();
        assert_eq!(
            registry.len(),
            13,
            "canonical registry must have exactly 13 UC entries"
        );
    }

    #[test]
    fn test_canonical_registry_is_mece() {
        // This panics if any uc_id or action_type is duplicated — proves MECE.
        let registry = AssertionRegistry::canonical();
        registry.validate_mece();
    }

    #[test]
    fn test_uc_for_action_known_actions() {
        let registry = AssertionRegistry::canonical();
        assert_eq!(
            registry.uc_for_action("governance_vote"),
            Some("UC-GOV-001")
        );
        assert_eq!(
            registry.uc_for_action("governance_proposal_create"),
            Some("UC-GOV-002")
        );
        assert_eq!(
            registry.uc_for_action("governance_finalize"),
            Some("UC-GOV-003")
        );
        assert_eq!(
            registry.uc_for_action("governance_execute"),
            Some("UC-GOV-004")
        );
        assert_eq!(registry.uc_for_action("gid_register"), Some("UC-GID-001"));
        assert_eq!(
            registry.uc_for_action("grim_trigger_check"),
            Some("UC-GID-002")
        );
        assert_eq!(
            registry.uc_for_action("apology_lifecycle"),
            Some("UC-GID-003")
        );
        assert_eq!(registry.uc_for_action("multisig_vote"), Some("UC-GID-004"));
        assert_eq!(
            registry.uc_for_action("dex_order_place"),
            Some("UC-DEX-001")
        );
        assert_eq!(
            registry.uc_for_action("dex_limit_order"),
            Some("UC-DEX-002")
        );
        assert_eq!(
            registry.uc_for_action("dex_order_cancel"),
            Some("UC-DEX-003")
        );
        assert_eq!(registry.uc_for_action("balance_query"), Some("UC-BAL-001"));
        assert_eq!(
            registry.uc_for_action("private_transfer"),
            Some("UC-TXN-001")
        );
    }

    #[test]
    fn test_uc_for_action_unknown_returns_none() {
        let registry = AssertionRegistry::canonical();
        assert_eq!(registry.uc_for_action("bogus_action"), None);
        assert_eq!(registry.uc_for_action(""), None);
    }

    #[test]
    #[should_panic(expected = "MECE violation")]
    fn test_validate_mece_panics_on_duplicate_uc_id() {
        let registry = AssertionRegistry {
            entries: vec![
                AssertionEntry {
                    uc_id: "UC-GOV-001",
                    action_type: "governance_vote",
                    description: "first",
                },
                AssertionEntry {
                    uc_id: "UC-GOV-001",
                    action_type: "other_action",
                    description: "duplicate uc_id",
                },
            ],
        };
        registry.validate_mece();
    }

    #[test]
    #[should_panic(expected = "MECE violation")]
    fn test_validate_mece_panics_on_duplicate_action_type() {
        let registry = AssertionRegistry {
            entries: vec![
                AssertionEntry {
                    uc_id: "UC-GOV-001",
                    action_type: "governance_vote",
                    description: "first",
                },
                AssertionEntry {
                    uc_id: "UC-GOV-002",
                    action_type: "governance_vote",
                    description: "duplicate action_type",
                },
            ],
        };
        registry.validate_mece();
    }
}
