pub mod fitness;
pub mod mutator;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A mutation strategy is a weighted probability distribution over mutation operators.
///
/// Each node independently evolves its strategy based on local feedback.
/// Strategies are shared via gossip to enable collective learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationStrategy {
    /// Weights per mutation type (normalized to sum to 1.0).
    pub weights: HashMap<MutationType, f64>,

    /// Contextual weights: different distributions for different fuzzing stages.
    pub stage_weights: HashMap<FuzzStage, HashMap<MutationType, f64>>,

    /// How many stacked mutations to apply in havoc mode.
    pub havoc_depth: u32,

    /// Probability of splicing two corpus entries.
    pub splice_probability: f64,

    /// Probability of using dictionary-based mutation.
    pub dict_probability: f64,

    /// Evolution generation counter.
    pub generation: u64,
}

/// Available mutation operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MutationType {
    BitFlip1,
    BitFlip2,
    BitFlip4,
    ByteFlip1,
    ByteFlip2,
    ByteFlip4,
    ArithAdd8,
    ArithAdd16,
    ArithAdd32,
    ArithSub8,
    ArithSub16,
    ArithSub32,
    InterestingValue8,
    InterestingValue16,
    InterestingValue32,
    RandomByte,
    DeleteBlock,
    InsertBlock,
    OverwriteBlock,
    Splice,
    DictionaryInsert,
    DictionaryOverwrite,
}

impl MutationType {
    /// All available mutation types.
    pub fn all() -> &'static [MutationType] {
        use MutationType::*;
        &[
            BitFlip1, BitFlip2, BitFlip4,
            ByteFlip1, ByteFlip2, ByteFlip4,
            ArithAdd8, ArithAdd16, ArithAdd32,
            ArithSub8, ArithSub16, ArithSub32,
            InterestingValue8, InterestingValue16, InterestingValue32,
            RandomByte,
            DeleteBlock, InsertBlock, OverwriteBlock,
            Splice,
            DictionaryInsert, DictionaryOverwrite,
        ]
    }
}

/// Fuzzing stages that may use different mutation distributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FuzzStage {
    /// Deterministic stage: systematic mutations.
    Deterministic,
    /// Havoc stage: random stacked mutations.
    Havoc,
    /// Splicing stage: combining corpus entries.
    Splicing,
}

impl MutationStrategy {
    /// Create a default strategy with uniform weights.
    pub fn default_uniform() -> Self {
        let types = MutationType::all();
        let uniform_weight = 1.0 / types.len() as f64;
        let weights: HashMap<_, _> = types.iter().map(|&t| (t, uniform_weight)).collect();

        Self {
            weights,
            stage_weights: HashMap::new(),
            havoc_depth: 4,
            splice_probability: 0.1,
            dict_probability: 0.05,
            generation: 0,
        }
    }

    /// Select a mutation type based on current weights.
    pub fn select_mutation(&self) -> MutationType {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let roll: f64 = rng.r#gen();

        let mut cumulative = 0.0;
        for (&mutation_type, &weight) in &self.weights {
            cumulative += weight;
            if roll < cumulative {
                return mutation_type;
            }
        }

        // Fallback (shouldn't happen with normalized weights)
        MutationType::RandomByte
    }

    /// Normalize weights to sum to 1.0.
    pub fn normalize(&mut self) {
        let sum: f64 = self.weights.values().sum();
        if sum > 0.0 {
            for weight in self.weights.values_mut() {
                *weight /= sum;
            }
        }
    }

    /// Blend another strategy into this one (for gossip-based learning).
    /// `alpha` controls blend ratio: 0.0 = keep ours, 1.0 = adopt theirs.
    pub fn blend(&mut self, other: &MutationStrategy, alpha: f64) {
        let alpha = alpha.clamp(0.0, 1.0);
        for (&mutation_type, weight) in &mut self.weights {
            if let Some(&other_weight) = other.weights.get(&mutation_type) {
                *weight = *weight * (1.0 - alpha) + other_weight * alpha;
            }
        }
        self.normalize();
        self.generation += 1;
    }

    /// Add random noise to weights (for diversity enforcement).
    pub fn add_noise(&mut self, magnitude: f64) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for weight in self.weights.values_mut() {
            let noise: f64 = rng.r#gen_range(-magnitude..magnitude);
            *weight = (*weight + noise).max(0.01); // floor
        }
        self.normalize();
        self.generation += 1;
    }
}
