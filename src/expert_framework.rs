/// Expert framework — unified interface for all domain experts.
///
/// Each expert (bedrock, biome, damage, audio, etc.) conforms to this trait.
/// Experts are loaded at startup, cached by tier (Hot/Warm/Cold), and called
/// during relevant game loops with deterministic int32 arithmetic.
///
/// Invention #101-ExpertFramework: Structural unification for all neural experts.

use std::collections::HashMap;

/// Input shape varies by domain, but all are 13-lane S13 vectors + optional context.
pub type Coordinate13Input = [i8; 13];

/// Expert output: scalar, categorical (0-N), or structured verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpertOutput {
    /// Structural verdict: 0=Void, 1=Matter, 2=Bedrock
    Structural(u8),
    /// Biome classification: 0=Temperate, 1=Desert, 2=Tundra, 3=Jungle, etc.
    Biome(u8),
    /// Ore tier: 0=copper, 1=iron, 2=gold, 3=rare, etc.
    Ore(u8),
    /// Monster spawn type: 0=goblin, 1=orc, 2=dragon, etc.
    Spawn(u8),
    /// Scalar output: damage, effectiveness %, difficulty factor (stored as i32, divide by 256 for f32)
    Scalar(i32),
    /// Paired output: (primary, secondary) for complex decisions
    Pair(u8, u8),
}

/// Weights and biases for a linear expert model.
/// All weights are i32, scaled by 2^8 for fixed-point arithmetic.
#[derive(Debug, Clone)]
pub struct ExpertWeights {
    /// (output_classes, 13) for linear layer, or (hidden, 13) if hidden layer exists
    pub weights: Vec<Vec<i32>>,
    /// Biases per output class (or per hidden if 2-layer model)
    pub biases: Vec<i32>,
    /// Number of hidden units (0 if linear-only)
    pub hidden_dim: usize,
    /// Optional second-layer weights if hidden_dim > 0: (output_classes, hidden_dim)
    pub weights_2: Option<Vec<Vec<i32>>>,
    /// Optional second-layer biases if hidden_dim > 0
    pub biases_2: Option<Vec<i32>>,
    /// Scale shift for fixed-point: all weights are scaled by 2^scale_shift
    pub scale_shift: u32,
}

/// A loaded expert, ready for inference.
pub struct Expert {
    /// Static registry key.
    pub name: &'static str,
    /// Domain this expert scores for.
    pub domain: ExpertDomain,
    /// Fixed-point weights/biases.
    pub weights: ExpertWeights,
}

/// Domain an expert scores for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExpertDomain {
    /// bedrock, biome, ore, spawning
    Worldgen,
    /// damage, loot, skill, difficulty
    Combat,
    /// audio, visual, appearance
    Sensory,
    /// content filter, anomaly, behavior
    Safety,
    /// narrative, relationship, puzzle, emergence
    Research,
}

impl Expert {
    /// Infer via linear layer: scores = dot_i32(s13, weights) + bias
    pub fn infer_linear(&self, input: &Coordinate13Input) -> i32 {
        let mut acc = 0i32;
        for (i, &trit) in input.iter().enumerate() {
            acc += (trit as i32) * self.weights.weights[0][i];
        }
        acc + self.weights.biases[0]
    }

    /// Infer via 2-layer model: hidden = ReLU(linear1(input)), then linear2(hidden)
    pub fn infer_2layer(&self, input: &Coordinate13Input) -> i32 {
        if self.weights.hidden_dim == 0 {
            return self.infer_linear(input);
        }

        let weights_2 = self.weights.weights_2.as_ref().expect("2-layer model missing weights_2");
        let biases_2 = self.weights.biases_2.as_ref().expect("2-layer model missing biases_2");

        // First layer: dot product + bias, then ReLU
        let mut hidden = vec![0i32; self.weights.hidden_dim];
        for h in 0..self.weights.hidden_dim {
            let mut acc = self.weights.biases[h];
            for (i, &trit) in input.iter().enumerate() {
                acc += (trit as i32) * self.weights.weights[h][i];
            }
            hidden[h] = acc.max(0); // ReLU
        }

        // Second layer: dot product with hidden activations
        let mut result = biases_2[0];
        for (h, &val) in hidden.iter().enumerate() {
            result += val * weights_2[0][h];
        }
        result
    }

    /// Infer all 3 outputs for classification (e.g., bedrock: Void/Matter/Bedrock)
    pub fn infer_all_classes(&self, input: &Coordinate13Input, num_classes: usize) -> Vec<i32> {
        if self.weights.hidden_dim == 0 {
            // Linear model: compute all class scores directly
            let mut scores = vec![0i32; num_classes];
            for class in 0..num_classes {
                let mut acc = self.weights.biases[class];
                for (i, &trit) in input.iter().enumerate() {
                    acc += (trit as i32) * self.weights.weights[class][i];
                }
                scores[class] = acc;
            }
            scores
        } else {
            // 2-layer model: compute hidden activations, then all outputs
            let weights_2 = self.weights.weights_2.as_ref().expect("2-layer model");
            let biases_2 = self.weights.biases_2.as_ref().expect("2-layer model");

            let mut hidden = vec![0i32; self.weights.hidden_dim];
            for h in 0..self.weights.hidden_dim {
                let mut acc = self.weights.biases[h];
                for (i, &trit) in input.iter().enumerate() {
                    acc += (trit as i32) * self.weights.weights[h][i];
                }
                hidden[h] = acc.max(0);
            }

            let mut scores = vec![0i32; num_classes];
            for class in 0..num_classes {
                let mut acc = biases_2[class];
                for (h, &val) in hidden.iter().enumerate() {
                    acc += val * weights_2[class][h];
                }
                scores[class] = acc;
            }
            scores
        }
    }
}

/// Registry of all loaded experts, organized by domain.
pub struct ExpertRegistry {
    /// All experts by name.
    pub experts: HashMap<String, Expert>,
    /// Expert names grouped by domain.
    pub by_domain: HashMap<ExpertDomain, Vec<String>>,
}

impl ExpertRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            experts: HashMap::new(),
            by_domain: HashMap::new(),
        }
    }

    /// Insert an expert, indexed under its domain.
    pub fn register(&mut self, name: &'static str, expert: Expert) {
        let domain = expert.domain;
        self.experts.insert(name.to_string(), expert);
        self.by_domain.entry(domain).or_insert_with(Vec::new).push(name.to_string());
    }

    /// Look up an expert by name.
    pub fn get(&self, name: &str) -> Option<&Expert> {
        self.experts.get(name)
    }

    /// All experts registered under a domain.
    pub fn by_domain(&self, domain: ExpertDomain) -> Vec<&Expert> {
        self.by_domain
            .get(&domain)
            .map(|names| names.iter().filter_map(|n| self.experts.get(n)).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_inference_deterministic() {
        let weights = ExpertWeights {
            weights: vec![vec![100, 50, 25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
            biases: vec![10],
            hidden_dim: 0,
            weights_2: None,
            biases_2: None,
            scale_shift: 8,
        };

        let expert = Expert {
            name: "test_linear",
            domain: ExpertDomain::Worldgen,
            weights,
        };

        let input = [1i8, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let score = expert.infer_linear(&input);
        assert_eq!(score, 100 + 50 + 25 + 10); // 185
    }

    #[test]
    fn all_classes_inference() {
        let weights = ExpertWeights {
            weights: vec![
                vec![100, 50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // class 0
                vec![50, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],  // class 1
                vec![-100, -100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // class 2
            ],
            biases: vec![10, 20, -50],
            hidden_dim: 0,
            weights_2: None,
            biases_2: None,
            scale_shift: 8,
        };

        let expert = Expert {
            name: "test_3class",
            domain: ExpertDomain::Worldgen,
            weights,
        };

        let input = [1i8, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let scores = expert.infer_all_classes(&input, 3);
        assert_eq!(scores[0], 100 + 50 + 10);      // 160
        assert_eq!(scores[1], 50 + 100 + 20);      // 170
        assert_eq!(scores[2], -100 + -100 + -50);  // -250
    }
}
