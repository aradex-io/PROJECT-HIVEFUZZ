use std::collections::HashMap;

use super::MutationType;

/// Tracks per-mutation-type fitness for strategy evolution.
///
/// Uses a rolling window to adapt to the changing coverage frontier.
pub struct FitnessTracker {
    /// Per-mutation stats in the current window.
    stats: HashMap<MutationType, MutationStats>,
    /// Rolling window size (in executions).
    window_size: u64,
    /// Total executions in current window.
    window_executions: u64,
    /// Weight for new edges in fitness calculation.
    edge_weight: f64,
    /// Weight for crashes in fitness calculation.
    crash_weight: f64,
}

#[derive(Debug, Clone, Default)]
struct MutationStats {
    executions: u64,
    new_edges_found: u64,
    crashes_found: u64,
}

impl FitnessTracker {
    pub fn new(window_size: u64) -> Self {
        Self {
            stats: HashMap::new(),
            window_size,
            window_executions: 0,
            edge_weight: 1.0,
            crash_weight: 10.0,
        }
    }

    /// Record the result of a fuzzing execution.
    pub fn record(&mut self, mutation: MutationType, new_edges: u32, crashed: bool) {
        let stats = self.stats.entry(mutation).or_default();
        stats.executions += 1;
        stats.new_edges_found += new_edges as u64;
        if crashed {
            stats.crashes_found += 1;
        }
        self.window_executions += 1;

        // Reset window if exceeded
        if self.window_executions >= self.window_size {
            self.reset_window();
        }
    }

    /// Get the fitness score for a mutation type.
    /// fitness = (new_edges * edge_weight + crashes * crash_weight) / executions
    pub fn fitness(&self, mutation: &MutationType) -> f64 {
        let Some(stats) = self.stats.get(mutation) else {
            return 0.0;
        };
        if stats.executions == 0 {
            return 0.0;
        }

        (stats.new_edges_found as f64 * self.edge_weight
            + stats.crashes_found as f64 * self.crash_weight)
            / stats.executions as f64
    }

    /// Get fitness scores for all tracked mutations.
    pub fn all_fitness(&self) -> HashMap<MutationType, f64> {
        self.stats
            .keys()
            .map(|&m| (m, self.fitness(&m)))
            .collect()
    }

    /// Reset the rolling window statistics.
    fn reset_window(&mut self) {
        // Decay stats rather than zeroing — preserve some history
        for stats in self.stats.values_mut() {
            stats.executions /= 2;
            stats.new_edges_found /= 2;
            stats.crashes_found /= 2;
        }
        self.window_executions = 0;
    }
}

/// Implements Exp3 (Exponential-weight algorithm for Exploration and Exploitation)
/// for mutation strategy weight updates.
pub struct Exp3Updater {
    /// Learning rate (gamma).
    gamma: f64,
    /// Minimum weight floor to prevent strategy abandonment.
    min_weight: f64,
}

impl Exp3Updater {
    pub fn new(gamma: f64, min_weight: f64) -> Self {
        Self { gamma, min_weight }
    }

    /// Update strategy weights based on fitness tracker data.
    pub fn update_weights(
        &self,
        weights: &mut HashMap<MutationType, f64>,
        fitness: &HashMap<MutationType, f64>,
    ) {
        let n = weights.len() as f64;

        for (mutation, weight) in weights.iter_mut() {
            let reward = fitness.get(mutation).copied().unwrap_or(0.0);
            // Exp3 update: w_i *= exp(gamma * reward / (n * p_i))
            let estimated_reward = reward / (n * *weight);
            *weight *= (self.gamma * estimated_reward).exp();
            *weight = weight.max(self.min_weight);
        }

        // Normalize
        let sum: f64 = weights.values().sum();
        if sum > 0.0 {
            for weight in weights.values_mut() {
                *weight /= sum;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fitness_tracking() {
        let mut tracker = FitnessTracker::new(100_000);

        // BitFlip1 finds edges
        for _ in 0..100 {
            tracker.record(MutationType::BitFlip1, 1, false);
        }

        // RandomByte finds nothing
        for _ in 0..100 {
            tracker.record(MutationType::RandomByte, 0, false);
        }

        assert!(tracker.fitness(&MutationType::BitFlip1) > tracker.fitness(&MutationType::RandomByte));
    }

    #[test]
    fn test_exp3_update() {
        let updater = Exp3Updater::new(0.1, 0.01);
        let mut weights: HashMap<MutationType, f64> = HashMap::new();
        weights.insert(MutationType::BitFlip1, 0.5);
        weights.insert(MutationType::RandomByte, 0.5);

        let mut fitness: HashMap<MutationType, f64> = HashMap::new();
        fitness.insert(MutationType::BitFlip1, 1.0);
        fitness.insert(MutationType::RandomByte, 0.0);

        updater.update_weights(&mut weights, &fitness);

        // BitFlip1 should now have higher weight
        assert!(weights[&MutationType::BitFlip1] > weights[&MutationType::RandomByte]);
    }
}
