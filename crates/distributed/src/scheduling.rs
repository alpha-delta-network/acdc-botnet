/// Advanced distributed scheduling with role affinity and load balancing
use crate::proto::*;
use crate::registry::WorkerRegistry;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Worker capabilities for role-based affinity
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkerCapability {
    Trader,
    Validator,
    Governor,
    Prover,      // Requires GPU
    GeneralUser,
}

impl WorkerCapability {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trader" => Some(Self::Trader),
            "validator" => Some(Self::Validator),
            "governor" => Some(Self::Governor),
            "prover" => Some(Self::Prover),
            "user" | "general_user" => Some(Self::GeneralUser),
            _ => None,
        }
    }
}

/// Scheduling strategy for bot distribution
#[derive(Debug, Clone)]
pub enum SchedulingStrategy {
    /// Distribute evenly across all workers
    RoundRobin,
    /// Assign based on worker capabilities
    RoleBased,
    /// Balance by current load
    LoadBalanced,
    /// Keep related bots together
    Affinity { group_by: String },
}

/// Advanced scheduler with role affinity and load balancing
pub struct AdvancedScheduler {
    registry: Arc<WorkerRegistry>,
    strategy: SchedulingStrategy,
}

impl AdvancedScheduler {
    pub fn new(registry: Arc<WorkerRegistry>, strategy: SchedulingStrategy) -> Self {
        Self { registry, strategy }
    }

    /// Schedule bots across workers using configured strategy
    pub fn schedule(&self, bot_specs: Vec<BotSpec>) -> Result<Vec<WorkerAssignment>> {
        match &self.strategy {
            SchedulingStrategy::RoundRobin => self.schedule_round_robin(bot_specs),
            SchedulingStrategy::RoleBased => self.schedule_role_based(bot_specs),
            SchedulingStrategy::LoadBalanced => self.schedule_load_balanced(bot_specs),
            SchedulingStrategy::Affinity { group_by } => {
                self.schedule_affinity(bot_specs, group_by)
            }
        }
    }

    /// Round-robin distribution
    fn schedule_round_robin(&self, bot_specs: Vec<BotSpec>) -> Result<Vec<WorkerAssignment>> {
        let workers = self.registry.list_workers();
        if workers.is_empty() {
            anyhow::bail!("No workers available");
        }

        let mut assignments: HashMap<String, Vec<BotSpec>> = HashMap::new();
        let mut worker_idx = 0;

        for bot_spec in bot_specs {
            let worker = &workers[worker_idx % workers.len()];
            assignments
                .entry(worker.worker_id.clone())
                .or_insert_with(Vec::new)
                .push(bot_spec);
            worker_idx += 1;
        }

        Ok(assignments
            .into_iter()
            .map(|(worker_id, bot_specs)| WorkerAssignment {
                worker_id,
                bot_specs,
            })
            .collect())
    }

    /// Role-based distribution (affinity)
    fn schedule_role_based(&self, bot_specs: Vec<BotSpec>) -> Result<Vec<WorkerAssignment>> {
        let workers = self.registry.list_workers();
        if workers.is_empty() {
            anyhow::bail!("No workers available");
        }

        // Build capability map
        let mut capability_map: HashMap<WorkerCapability, Vec<String>> = HashMap::new();
        for worker in &workers {
            for cap_str in &worker.capabilities {
                if let Some(cap) = WorkerCapability::from_string(cap_str) {
                    capability_map
                        .entry(cap)
                        .or_insert_with(Vec::new)
                        .push(worker.worker_id.clone());
                }
            }
        }

        let mut assignments: HashMap<String, Vec<BotSpec>> = HashMap::new();

        for bot_spec in bot_specs {
            // Determine bot's required capability
            let required_cap = match bot_spec.role.as_str() {
                "trader" => WorkerCapability::Trader,
                "validator" => WorkerCapability::Validator,
                "governor" => WorkerCapability::Governor,
                "prover" => WorkerCapability::Prover,
                _ => WorkerCapability::GeneralUser,
            };

            // Find workers with this capability
            let eligible_workers = capability_map
                .get(&required_cap)
                .or_else(|| capability_map.get(&WorkerCapability::GeneralUser))
                .ok_or_else(|| anyhow::anyhow!("No workers with capability {:?}", required_cap))?;

            // Assign to least-loaded eligible worker
            let worker_id = self.find_least_loaded_worker(eligible_workers, &assignments);

            assignments
                .entry(worker_id)
                .or_insert_with(Vec::new)
                .push(bot_spec);
        }

        Ok(assignments
            .into_iter()
            .map(|(worker_id, bot_specs)| WorkerAssignment {
                worker_id,
                bot_specs,
            })
            .collect())
    }

    /// Load-balanced distribution
    fn schedule_load_balanced(&self, bot_specs: Vec<BotSpec>) -> Result<Vec<WorkerAssignment>> {
        let workers = self.registry.list_workers();
        if workers.is_empty() {
            anyhow::bail!("No workers available");
        }

        let mut assignments: HashMap<String, Vec<BotSpec>> = HashMap::new();

        for bot_spec in bot_specs {
            // Find worker with most available capacity
            let worker_id = self.find_most_available_worker(&workers, &assignments)?;

            assignments
                .entry(worker_id)
                .or_insert_with(Vec::new)
                .push(bot_spec);
        }

        Ok(assignments
            .into_iter()
            .map(|(worker_id, bot_specs)| WorkerAssignment {
                worker_id,
                bot_specs,
            })
            .collect())
    }

    /// Affinity-based distribution (keep related bots together)
    fn schedule_affinity(
        &self,
        bot_specs: Vec<BotSpec>,
        group_by: &str,
    ) -> Result<Vec<WorkerAssignment>> {
        let workers = self.registry.list_workers();
        if workers.is_empty() {
            anyhow::bail!("No workers available");
        }

        // Group bots by affinity key
        let mut groups: HashMap<String, Vec<BotSpec>> = HashMap::new();
        for bot_spec in bot_specs {
            let key = self.extract_affinity_key(&bot_spec, group_by);
            groups.entry(key).or_insert_with(Vec::new).push(bot_spec);
        }

        // Assign each group to a single worker
        let mut assignments: HashMap<String, Vec<BotSpec>> = HashMap::new();
        let mut worker_idx = 0;

        for (_, group_bots) in groups {
            let worker = &workers[worker_idx % workers.len()];
            assignments
                .entry(worker.worker_id.clone())
                .or_insert_with(Vec::new)
                .extend(group_bots);
            worker_idx += 1;
        }

        Ok(assignments
            .into_iter()
            .map(|(worker_id, bot_specs)| WorkerAssignment {
                worker_id,
                bot_specs,
            })
            .collect())
    }

    /// Find least-loaded worker from eligible list
    fn find_least_loaded_worker(
        &self,
        eligible: &[String],
        current_assignments: &HashMap<String, Vec<BotSpec>>,
    ) -> String {
        eligible
            .iter()
            .min_by_key(|worker_id| current_assignments.get(*worker_id).map_or(0, |v| v.len()))
            .cloned()
            .unwrap_or_else(|| eligible[0].clone())
    }

    /// Find worker with most available capacity
    fn find_most_available_worker(
        &self,
        workers: &[WorkerInfo],
        current_assignments: &HashMap<String, Vec<BotSpec>>,
    ) -> Result<String> {
        workers
            .iter()
            .max_by_key(|worker| {
                let assigned = current_assignments
                    .get(&worker.worker_id)
                    .map_or(0, |v| v.len());
                worker.max_bots as usize - assigned
            })
            .map(|w| w.worker_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No available workers"))
    }

    /// Extract affinity key from bot spec
    fn extract_affinity_key(&self, bot_spec: &BotSpec, group_by: &str) -> String {
        match group_by {
            "role" => bot_spec.role.clone(),
            "scenario" => {
                // Extract scenario from tags
                bot_spec
                    .tags
                    .iter()
                    .find(|t| t.starts_with("scenario:"))
                    .cloned()
                    .unwrap_or_else(|| "default".to_string())
            }
            _ => "default".to_string(),
        }
    }
}

/// Isolation policy for Byzantine behaviors
pub struct IsolationPolicy {
    isolated_workers: Vec<String>,
}

impl IsolationPolicy {
    pub fn new() -> Self {
        Self {
            isolated_workers: Vec::new(),
        }
    }

    /// Check if bot requires isolation
    pub fn requires_isolation(&self, bot_spec: &BotSpec) -> bool {
        bot_spec
            .tags
            .iter()
            .any(|t| t == "byzantine" || t == "adversarial")
    }

    /// Get or create isolated worker for Byzantine bots
    pub fn get_isolated_worker(&mut self, workers: &[WorkerInfo]) -> Option<String> {
        // Use dedicated isolated worker if available
        self.isolated_workers.first().cloned().or_else(|| {
            // Otherwise, use any available worker and mark as isolated
            workers.first().map(|w| {
                self.isolated_workers.push(w.worker_id.clone());
                w.worker_id.clone()
            })
        })
    }
}

impl Default for IsolationPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_capability_parsing() {
        assert_eq!(
            WorkerCapability::from_string("trader"),
            Some(WorkerCapability::Trader)
        );
        assert_eq!(
            WorkerCapability::from_string("prover"),
            Some(WorkerCapability::Prover)
        );
        assert_eq!(WorkerCapability::from_string("unknown"), None);
    }
}
