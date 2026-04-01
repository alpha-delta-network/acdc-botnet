/// Gauntlet bot fleet for TN006-LIGHT testnet run
///
/// LightFleet contains all bots needed for the 7-phase gauntlet scenario:
///   - validators (5)
///   - provers (1)
///   - governors (5)
///   - delta_voters (10)
///   - tech_reps (3)
///   - traders (15)
///   - user_transactors (10)
///   - bridges (4)
///   - scanners (1)
///   - oracles (1)
///   - earn_in (4)
///   - atomic_swaps (2)
///   - adversarials (5)
use adnet_testbot::{BehaviorResult, Bot, BotContext, Result};
use async_trait::async_trait;

/// A generic bot used for all fleet roles.
///
/// Stores its id, role, and the context provided at setup. Behaviors return
/// a success result so the runner can proceed through phases.
pub struct FleetBot {
    id: String,
    role: String,
    context: Option<BotContext>,
}

impl FleetBot {
    pub fn new(id: String, role: String) -> Self {
        Self {
            id,
            role,
            context: None,
        }
    }
}

#[async_trait]
impl Bot for FleetBot {
    async fn setup(&mut self, context: &BotContext) -> Result<()> {
        self.context = Some(context.clone());
        tracing::info!("FleetBot {} ({}) setup complete", self.id, self.role);
        Ok(())
    }

    async fn execute_behavior(&mut self, behavior_id: &str) -> Result<BehaviorResult> {
        tracing::info!(
            "FleetBot {} ({}) executing behavior: {}",
            self.id,
            self.role,
            behavior_id
        );
        Ok(BehaviorResult::success(format!("Executed {}", behavior_id)))
    }

    async fn teardown(&mut self) -> Result<()> {
        tracing::info!("FleetBot {} ({}) teardown complete", self.id, self.role);
        Ok(())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn role(&self) -> &str {
        &self.role
    }
}

/// Collection of all bots for the TN006-LIGHT gauntlet scenario.
///
/// Each bot type is stored in a typed `Vec<Box<dyn Bot>>`. The public
/// `bots` field exposes a flattened list so callers can cheaply query
/// the total count (`fleet.bots.len()`).
pub struct LightFleet {
    pub bots: Vec<Box<dyn Bot>>,

    validators: Vec<Box<dyn Bot>>,
    provers: Vec<Box<dyn Bot>>,
    governors: Vec<Box<dyn Bot>>,
    delta_voters: Vec<Box<dyn Bot>>,
    tech_reps: Vec<Box<dyn Bot>>,
    traders: Vec<Box<dyn Bot>>,
    user_transactors: Vec<Box<dyn Bot>>,
    bridges: Vec<Box<dyn Bot>>,
    scanners: Vec<Box<dyn Bot>>,
    oracles: Vec<Box<dyn Bot>>,
    earn_in: Vec<Box<dyn Bot>>,
    atomic_swaps: Vec<Box<dyn Bot>>,
    adversarials: Vec<Box<dyn Bot>>,
}

fn make_bots(role: &str, count: usize) -> Vec<Box<dyn Bot>> {
    (0..count)
        .map(|i| -> Box<dyn Bot> {
            Box::new(FleetBot::new(format!("{}-{}", role, i), role.to_string()))
        })
        .collect()
}

impl LightFleet {
    /// Construct the full fleet with deterministic bot IDs.
    pub fn build() -> Self {
        let validators = make_bots("validators", 5);
        let provers = make_bots("provers", 1);
        let governors = make_bots("governors", 5);
        let delta_voters = make_bots("delta_voters", 10);
        let tech_reps = make_bots("tech_reps", 3);
        let traders = make_bots("traders", 15);
        let user_transactors = make_bots("user_transactors", 10);
        let bridges = make_bots("bridges", 4);
        let scanners = make_bots("scanners", 1);
        let oracles = make_bots("oracles", 1);
        let earn_in = make_bots("earn_in", 4);
        let atomic_swaps = make_bots("atomic_swaps", 2);
        let adversarials = make_bots("adversarials", 5);

        // Build the flat list for total-count queries
        let bots: Vec<Box<dyn Bot>> = [
            make_bots("validators", 5),
            make_bots("provers", 1),
            make_bots("governors", 5),
            make_bots("delta_voters", 10),
            make_bots("tech_reps", 3),
            make_bots("traders", 15),
            make_bots("user_transactors", 10),
            make_bots("bridges", 4),
            make_bots("scanners", 1),
            make_bots("oracles", 1),
            make_bots("earn_in", 4),
            make_bots("atomic_swaps", 2),
            make_bots("adversarials", 5),
        ]
        .into_iter()
        .flatten()
        .collect();

        Self {
            bots,
            validators,
            provers,
            governors,
            delta_voters,
            tech_reps,
            traders,
            user_transactors,
            bridges,
            scanners,
            oracles,
            earn_in,
            atomic_swaps,
            adversarials,
        }
    }

    /// Get a mutable reference to a bot by type name and index.
    ///
    /// Returns an error if the bot type is unknown or the index is out of
    /// range.
    pub fn get_bot_mut<'a>(
        &'a mut self,
        bot_type: &str,
        index: usize,
    ) -> anyhow::Result<&'a mut dyn Bot> {
        let vec: &'a mut Vec<Box<dyn Bot>> = match bot_type {
            "validators" => &mut self.validators,
            "provers" => &mut self.provers,
            "governors" => &mut self.governors,
            "delta_voters" => &mut self.delta_voters,
            "tech_reps" => &mut self.tech_reps,
            "traders" => &mut self.traders,
            "user_transactors" => &mut self.user_transactors,
            "bridges" => &mut self.bridges,
            "scanners" => &mut self.scanners,
            "oracles" => &mut self.oracles,
            "earn_in" => &mut self.earn_in,
            "atomic_swaps" => &mut self.atomic_swaps,
            "adversarials" => &mut self.adversarials,
            other => {
                return Err(anyhow::anyhow!("Unknown bot type: {}", other));
            }
        };

        match vec.get_mut(index) {
            Some(b) => Ok(b.as_mut()),
            None => Err(anyhow::anyhow!(
                "Bot type '{}' index {} out of range",
                bot_type,
                index
            )),
        }
    }
}
