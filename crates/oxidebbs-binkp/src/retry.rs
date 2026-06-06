use std::time::Duration;

use crate::BinkpError;

/// Exponential backoff policy for BinkP poll attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinkpRetryPolicy {
    /// Maximum number of total attempts, including the initial try.
    pub max_attempts: u32,
    /// Delay after the first failed attempt.
    pub initial_delay: Duration,
    /// Maximum delay between attempts.
    pub max_delay: Duration,
    /// Multiplicative backoff factor.
    pub multiplier: u32,
}

impl BinkpRetryPolicy {
    /// Build a validated retry policy.
    ///
    /// # Errors
    ///
    /// Returns a protocol error when attempts, delays, or multiplier values are
    /// nonsensical.
    pub fn new(
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: u32,
    ) -> Result<Self, BinkpError> {
        if max_attempts == 0 {
            return Err(BinkpError::Protocol(
                "BinkP retry policy requires at least one attempt".to_string(),
            ));
        }
        if initial_delay.is_zero() {
            return Err(BinkpError::Protocol(
                "BinkP retry policy requires a nonzero initial delay".to_string(),
            ));
        }
        if max_delay < initial_delay {
            return Err(BinkpError::Protocol(
                "BinkP retry policy max delay must be at least the initial delay".to_string(),
            ));
        }
        if multiplier < 1 {
            return Err(BinkpError::Protocol(
                "BinkP retry policy multiplier must be at least 1".to_string(),
            ));
        }

        Ok(Self {
            max_attempts,
            initial_delay,
            max_delay,
            multiplier,
        })
    }

    /// Return true when another attempt is allowed after `failed_attempts`.
    #[must_use]
    pub fn should_retry_after(self, failed_attempts: u32) -> bool {
        failed_attempts < self.max_attempts
    }

    /// Return the delay after a failed attempt, or `None` when retry is done.
    ///
    /// `failed_attempt` is one-based: `1` means the first attempt has failed and
    /// the returned delay precedes attempt 2.
    #[must_use]
    pub fn delay_after_failure(self, failed_attempt: u32) -> Option<Duration> {
        if failed_attempt == 0 || failed_attempt >= self.max_attempts {
            return None;
        }

        let mut delay = self.initial_delay;
        for _ in 1..failed_attempt {
            delay = delay.saturating_mul(self.multiplier);
            if delay >= self.max_delay {
                return Some(self.max_delay);
            }
        }

        Some(delay.min(self.max_delay))
    }
}

impl Default for BinkpRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(30 * 60),
            multiplier: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_uses_exponential_backoff() {
        let policy = BinkpRetryPolicy::default();

        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.delay_after_failure(1), Some(Duration::from_secs(30)));
        assert_eq!(policy.delay_after_failure(2), Some(Duration::from_secs(60)));
        assert_eq!(
            policy.delay_after_failure(3),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            policy.delay_after_failure(4),
            Some(Duration::from_secs(240))
        );
        assert_eq!(policy.delay_after_failure(5), None);
    }

    #[test]
    fn retry_policy_caps_at_max_delay() {
        let policy = BinkpRetryPolicy::new(6, Duration::from_secs(10), Duration::from_secs(25), 2)
            .expect("policy");

        assert_eq!(policy.delay_after_failure(1), Some(Duration::from_secs(10)));
        assert_eq!(policy.delay_after_failure(2), Some(Duration::from_secs(20)));
        assert_eq!(policy.delay_after_failure(3), Some(Duration::from_secs(25)));
        assert_eq!(policy.delay_after_failure(4), Some(Duration::from_secs(25)));
    }

    #[test]
    fn retry_policy_reports_whether_more_attempts_remain() {
        let policy = BinkpRetryPolicy::new(3, Duration::from_secs(1), Duration::from_secs(10), 2)
            .expect("policy");

        assert!(policy.should_retry_after(1));
        assert!(policy.should_retry_after(2));
        assert!(!policy.should_retry_after(3));
    }

    #[test]
    fn retry_policy_rejects_invalid_values() {
        assert!(matches!(
            BinkpRetryPolicy::new(0, Duration::from_secs(1), Duration::from_secs(1), 1),
            Err(BinkpError::Protocol(_))
        ));
        assert!(matches!(
            BinkpRetryPolicy::new(1, Duration::ZERO, Duration::from_secs(1), 1),
            Err(BinkpError::Protocol(_))
        ));
        assert!(matches!(
            BinkpRetryPolicy::new(1, Duration::from_secs(2), Duration::from_secs(1), 1),
            Err(BinkpError::Protocol(_))
        ));
        assert!(matches!(
            BinkpRetryPolicy::new(1, Duration::from_secs(1), Duration::from_secs(1), 0),
            Err(BinkpError::Protocol(_))
        ));
    }
}
