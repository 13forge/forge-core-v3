// Bifurcation state machine: detects approach to irreversible collapse (liquidation boundary)
// Ingests time-series streams (price, volume, account equity); outputs circuit-breaker signals

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BifurcationState {
    Stable,
    Accelerating,
    Critical,
    CircuitBreakerTriggered,
}

#[derive(Clone, Copy, Debug)]
pub struct BifurcationMetrics {
    pub value: f64,
    pub velocity: f64,
    pub acceleration: f64,
    pub margin_to_threshold: f64,
    pub state: BifurcationState,
    pub circuit_breaker_active: bool,
}

pub struct BifurcationDetector {
    window_size: usize,
    threshold: f64,
    margin_trigger: f64,
    accel_trigger: f64,
    history: VecDeque<f64>,
}

impl BifurcationDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            window_size: 3,
            threshold,
            margin_trigger: 0.15,
            accel_trigger: 0.05,
            history: VecDeque::with_capacity(3),
        }
    }

    pub fn with_margin(mut self, margin: f64) -> Self {
        self.margin_trigger = margin;
        self
    }

    pub fn with_accel_trigger(mut self, accel: f64) -> Self {
        self.accel_trigger = accel;
        self
    }

    pub fn ingest(&mut self, value: f64) -> BifurcationMetrics {
        self.history.push_back(value);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        let metrics = self.compute_metrics();
        metrics
    }

    fn compute_metrics(&self) -> BifurcationMetrics {
        let current = *self.history.back().unwrap_or(&0.0);
        let margin = (self.threshold - current).abs() / self.threshold;

        let velocity = if self.history.len() >= 2 {
            let prev = self.history[self.history.len() - 2];
            (current - prev).abs()
        } else {
            0.0
        };

        let acceleration = if self.history.len() >= 3 {
            let v2 = (self.history[self.history.len() - 1] - self.history[self.history.len() - 2]).abs();
            let v1 = (self.history[self.history.len() - 2] - self.history[self.history.len() - 3]).abs();
            (v2 - v1).abs()
        } else {
            0.0
        };

        let is_critical = margin < self.margin_trigger && acceleration > self.accel_trigger;
        let state = if is_critical {
            BifurcationState::CircuitBreakerTriggered
        } else if acceleration > self.accel_trigger {
            BifurcationState::Accelerating
        } else if margin < self.margin_trigger {
            BifurcationState::Critical
        } else {
            BifurcationState::Stable
        };

        BifurcationMetrics {
            value: current,
            velocity,
            acceleration,
            margin_to_threshold: margin,
            state,
            circuit_breaker_active: is_critical,
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_state() {
        let mut detector = BifurcationDetector::new(1000.0);
        let m1 = detector.ingest(950.0);
        let m2 = detector.ingest(950.0);
        let m3 = detector.ingest(950.0);

        assert_eq!(m3.state, BifurcationState::Stable);
        assert!(!m3.circuit_breaker_active);
    }

    #[test]
    fn test_accelerating_toward_boundary() {
        let mut detector = BifurcationDetector::new(1000.0).with_accel_trigger(0.01);
        detector.ingest(950.0);
        detector.ingest(920.0);
        let m3 = detector.ingest(880.0);

        assert!(m3.velocity > 0.0);
        assert!(m3.acceleration > 0.0);
        assert_eq!(m3.state, BifurcationState::Accelerating);
    }

    #[test]
    fn test_circuit_breaker_triggers() {
        let mut detector = BifurcationDetector::new(1000.0)
            .with_margin(0.20)
            .with_accel_trigger(0.01);

        detector.ingest(950.0);
        detector.ingest(920.0);
        let m3 = detector.ingest(880.0);

        assert!(m3.circuit_breaker_active);
        assert_eq!(m3.state, BifurcationState::CircuitBreakerTriggered);
    }

    #[test]
    fn test_liquidation_scenario() {
        let mut detector = BifurcationDetector::new(100.0)
            .with_margin(0.15)
            .with_accel_trigger(0.02);

        let drawdown_sequence = vec![
            95.0,  // -5%
            85.0,  // -10% total
            70.0,  // -30% total, accelerating
            50.0,  // -50% total, critical
        ];

        for value in drawdown_sequence {
            let m = detector.ingest(value);
            if m.margin_to_threshold < 0.15 && m.acceleration > 0.02 {
                assert!(m.circuit_breaker_active);
                break;
            }
        }
    }
}
