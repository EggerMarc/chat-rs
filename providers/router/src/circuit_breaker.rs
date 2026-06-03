use std::time::{Duration, Instant};

/// Configuration for the circuit breaker.
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the circuit opens (skips provider).
    pub failure_threshold: u32,
    /// How long an open circuit waits before allowing a probe request.
    pub recovery_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
        }
    }
}

struct ProviderCircuit {
    consecutive_failures: u32,
    last_failure_at: Option<Instant>,
}

impl ProviderCircuit {
    fn new() -> Self {
        Self {
            consecutive_failures: 0,
            last_failure_at: None,
        }
    }
}

pub(crate) struct CircuitBreaker {
    config: CircuitBreakerConfig,
    circuits: Vec<ProviderCircuit>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig, provider_count: usize) -> Self {
        let circuits = (0..provider_count)
            .map(|_| ProviderCircuit::new())
            .collect();
        Self { config, circuits }
    }

    /// Returns true if the provider should be tried (closed or half-open).
    pub fn is_available(&self, idx: usize) -> bool {
        let circuit = &self.circuits[idx];
        if circuit.consecutive_failures < self.config.failure_threshold {
            return true;
        }
        match circuit.last_failure_at {
            Some(at) => at.elapsed() >= self.config.recovery_timeout,
            None => true,
        }
    }

    /// Returns the index of the provider whose circuit has been open the longest,
    /// i.e. the one most likely to have recovered. Used as a last resort when all
    /// circuits are open.
    pub fn longest_open(&self) -> Option<usize> {
        self.circuits
            .iter()
            .enumerate()
            .filter(|(_, c)| c.consecutive_failures >= self.config.failure_threshold)
            .min_by_key(|(_, c)| c.last_failure_at)
            .map(|(idx, _)| idx)
    }

    pub fn record_success(&mut self, idx: usize) {
        let circuit = &mut self.circuits[idx];
        circuit.consecutive_failures = 0;
        circuit.last_failure_at = None;
    }

    pub fn record_failure(&mut self, idx: usize) {
        let circuit = &mut self.circuits[idx];
        circuit.consecutive_failures += 1;
        circuit.last_failure_at = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_circuit_is_available() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_timeout: Duration::from_secs(30),
            },
            2,
        );
        assert!(cb.is_available(0));
        assert!(cb.is_available(1));
    }

    #[test]
    fn opens_after_threshold() {
        let mut cb = CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 2,
                recovery_timeout: Duration::from_secs(30),
            },
            1,
        );
        cb.record_failure(0);
        assert!(cb.is_available(0));
        cb.record_failure(0);
        assert!(!cb.is_available(0));
    }

    #[test]
    fn success_resets() {
        let mut cb = CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 2,
                recovery_timeout: Duration::from_secs(30),
            },
            1,
        );
        cb.record_failure(0);
        cb.record_failure(0);
        assert!(!cb.is_available(0));
        cb.record_success(0);
        assert!(cb.is_available(0));
    }

    #[test]
    fn half_open_after_recovery_timeout() {
        let mut cb = CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 1,
                recovery_timeout: Duration::from_millis(0),
            },
            1,
        );
        cb.record_failure(0);
        assert!(cb.is_available(0));
    }

    #[test]
    fn longest_open_picks_oldest_failure() {
        let mut cb = CircuitBreaker::new(
            CircuitBreakerConfig {
                failure_threshold: 1,
                recovery_timeout: Duration::from_secs(999),
            },
            3,
        );
        cb.record_failure(1);
        cb.record_failure(2);
        cb.record_failure(0);
        assert_eq!(cb.longest_open(), Some(1));
    }
}
