use core::fmt::Debug;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Defines the strategy to apply in case of a failure.
/// This is applied, for example, when a task execution fails
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct RetryStrategy {
    pub retry_policy: RetryPolicy,
    pub backoff_policy: BackoffPolicy,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            retry_policy: RetryPolicy::None,
            backoff_policy: BackoffPolicy::Fixed { secs: 2 },
        }
    }
}

impl RetryStrategy {
    /// Return whether a retry attempt should be performed and the backoff time
    pub fn should_retry(&self, failed_attempts: u32) -> (bool, Duration) {
        (
            self.retry_policy.should_retry(failed_attempts),
            self.backoff_policy.should_wait(failed_attempts),
        )
    }
}

// Defines the retry policy of a RetryStrategy
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum RetryPolicy {
    /// No Retry attempts defined
    None,
    /// The operation will be retried for a max number of times.
    MaxRetries { retries: u32 },
    /// The operation will be retried an infinite number of times.
    Infinite,
    // Timeout,
}

impl RetryPolicy {
    fn should_retry(&self, failed_attempts: u32) -> bool {
        if failed_attempts == 0 {
            true
        } else {
            match self {
                RetryPolicy::None => false,
                RetryPolicy::Infinite => true,
                RetryPolicy::MaxRetries { retries: attempts } => *attempts + 1 > failed_attempts,
            }
        }
    }
}

// Defines the backoff policy of a RetryStrategy
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum BackoffPolicy {
    /// No backoff, the retry will be attempted without waiting
    None,
    /// A fixed amount ot time will be waited between each retry attempt
    Fixed { secs: u32 },
    /// Permits to specify the amount of time between two consecutive retry attempts.
    /// The time to wait after 'i' retries is specified in the vector at position 'i'.
    /// If the number of retries is bigger than the vector length, then the last value in the vector is used.
    /// For example:
    /// secs = [1,2,6] -> It waits 1 second after the first failure, 2 seconds after the second failure and then 6 seconds for all following failures.
    Variable { secs: Vec<u32> },
    /// Implementation of BackoffPolicy that increases the back off period for each retry attempt in a given set using the exponential function.
    Exponential {
        /// The period to sleep on the first backoff.
        secs: u32,
        // The multiplier to use to generate the next backoff interval from the last.
        multiplier: u64,
    },
}

impl BackoffPolicy {
    /// Return the wait time before attempting a retry
    /// after the specified number of failed attempts
    fn should_wait(&self, failed_attempts: u32) -> Duration {
        if failed_attempts == 0 {
            Duration::ZERO
        } else {
            match self {
                BackoffPolicy::None => Duration::ZERO,
                BackoffPolicy::Fixed { secs } => {
                    if *secs > 0 {
                        Duration::from_secs(*secs as u64)
                    } else {
                        Duration::ZERO
                    }
                }
                BackoffPolicy::Variable { secs } => {
                    let index = (failed_attempts - 1) as usize;
                    let option_wait_secs = if secs.len() > index {
                        secs.get(index)
                    } else {
                        secs.last()
                    };
                    match option_wait_secs {
                        Some(wait_secs) => {
                            if *wait_secs > 0 {
                                Duration::from_secs(*wait_secs as u64)
                            } else {
                                Duration::ZERO
                            }
                        }
                        None => Duration::ZERO,
                    }
                }
                BackoffPolicy::Exponential { secs, multiplier } => {
                    if *secs > 0 {
                        let multiplier = multiplier.saturating_pow(failed_attempts - 1);
                        let wait_secs = multiplier.saturating_mul(*secs as u64);
                        Duration::from_secs(wait_secs)
                    } else {
                        Duration::ZERO
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub mod test {

    use super::*;

    #[test]
    fn retry_policy_none_should_never_retry() {
        assert!(RetryPolicy::None.should_retry(0));
        assert!(!RetryPolicy::None.should_retry(1));
        assert!(!RetryPolicy::None.should_retry(10));
        assert!(!RetryPolicy::None.should_retry(100));
    }

    #[test]
    fn retry_policy_max_should_return_when_to_retry() {
        assert!(RetryPolicy::MaxRetries { retries: 0 }.should_retry(0));
        assert!(!RetryPolicy::MaxRetries { retries: 0 }.should_retry(1));
        assert!(!RetryPolicy::MaxRetries { retries: 0 }.should_retry(10));
        assert!(!RetryPolicy::MaxRetries { retries: 0 }.should_retry(100));

        assert!(RetryPolicy::MaxRetries { retries: 1 }.should_retry(0));
        assert!(RetryPolicy::MaxRetries { retries: 1 }.should_retry(1));
        assert!(!RetryPolicy::MaxRetries { retries: 1 }.should_retry(2));
        assert!(!RetryPolicy::MaxRetries { retries: 1 }.should_retry(10));
        assert!(!RetryPolicy::MaxRetries { retries: 1 }.should_retry(100));

        assert!(RetryPolicy::MaxRetries { retries: 10 }.should_retry(0));
        assert!(RetryPolicy::MaxRetries { retries: 10 }.should_retry(1));
        assert!(RetryPolicy::MaxRetries { retries: 10 }.should_retry(10));
        assert!(!RetryPolicy::MaxRetries { retries: 10 }.should_retry(11));
        assert!(!RetryPolicy::MaxRetries { retries: 10 }.should_retry(100));
    }

    #[test]
    fn retry_policy_infinite_should_return_when_to_retry() {
        assert!(RetryPolicy::Infinite.should_retry(0));
        assert!(RetryPolicy::Infinite.should_retry(1));
        assert!(RetryPolicy::Infinite.should_retry(10));
        assert!(RetryPolicy::Infinite.should_retry(100));
    }

    #[test]
    fn backoff_policy_none_should_never_wait() {
        assert_eq!(Duration::ZERO, BackoffPolicy::None.should_wait(0));
        assert_eq!(Duration::ZERO, BackoffPolicy::None.should_wait(1));
        assert_eq!(Duration::ZERO, BackoffPolicy::None.should_wait(10));
        assert_eq!(Duration::ZERO, BackoffPolicy::None.should_wait(100));
    }

    #[test]
    fn backoff_policy_fixed_should_return_the_wait_time() {
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Fixed { secs: 100 }.should_wait(0)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Fixed { secs: 100 }.should_wait(1)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Fixed { secs: 100 }.should_wait(10)
        );
        assert_eq!(
            Duration::from_secs(1123),
            BackoffPolicy::Fixed { secs: 1123 }.should_wait(100)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Fixed { secs: 0 }.should_wait(0)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Fixed { secs: 0 }.should_wait(1)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Fixed { secs: 0 }.should_wait(10)
        );
    }

    #[test]
    fn backoff_policy_variable_should_return_the_wait_time() {
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!() }.should_wait(0)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!() }.should_wait(1)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!() }.should_wait(200)
        );

        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!(0) }.should_wait(0)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!(0) }.should_wait(1)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!(0) }.should_wait(100)
        );

        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable { secs: vec!(100) }.should_wait(0)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Variable { secs: vec!(100) }.should_wait(1)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Variable { secs: vec!(100) }.should_wait(2)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Variable { secs: vec!(100) }.should_wait(10)
        );
        assert_eq!(
            Duration::from_secs(100),
            BackoffPolicy::Variable { secs: vec!(100) }.should_wait(100)
        );

        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(0)
        );
        assert_eq!(
            Duration::from_secs(111),
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(1)
        );
        assert_eq!(
            Duration::from_secs(222),
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(2)
        );
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(3)
        );
        assert_eq!(
            Duration::from_secs(444),
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(4)
        );
        assert_eq!(
            Duration::from_secs(444),
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(5)
        );
        assert_eq!(
            Duration::from_secs(444),
            BackoffPolicy::Variable {
                secs: vec!(111, 222, 0, 444)
            }
            .should_wait(100_000)
        );
    }

    #[test]
    fn backoff_policy_exponential_should_return_the_wait_time() {
        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Exponential {
                secs: 123,
                multiplier: 2
            }
            .should_wait(0)
        );
        assert_eq!(
            Duration::from_secs(123),
            BackoffPolicy::Exponential {
                secs: 123,
                multiplier: 2
            }
            .should_wait(1)
        );
        assert_eq!(
            Duration::from_secs(246),
            BackoffPolicy::Exponential {
                secs: 123,
                multiplier: 2
            }
            .should_wait(2)
        );
        assert_eq!(
            Duration::from_secs(492),
            BackoffPolicy::Exponential {
                secs: 123,
                multiplier: 2
            }
            .should_wait(3)
        );

        assert_eq!(
            Duration::ZERO,
            BackoffPolicy::Exponential {
                secs: 1000,
                multiplier: 3
            }
            .should_wait(0)
        );
        assert_eq!(
            Duration::from_secs(1000),
            BackoffPolicy::Exponential {
                secs: 1000,
                multiplier: 3
            }
            .should_wait(1)
        );
        assert_eq!(
            Duration::from_secs(3000),
            BackoffPolicy::Exponential {
                secs: 1000,
                multiplier: 3
            }
            .should_wait(2)
        );
        assert_eq!(
            Duration::from_secs(9000),
            BackoffPolicy::Exponential {
                secs: 1000,
                multiplier: 3
            }
            .should_wait(3)
        );
    }

    #[test]
    fn retry_policy_should_return_whether_to_retry() {
        let retry_strategy = RetryStrategy {
            retry_policy: RetryPolicy::MaxRetries { retries: 1 },
            backoff_policy: BackoffPolicy::Fixed { secs: 34 },
        };
        assert_eq!((true, Duration::ZERO), retry_strategy.should_retry(0));
        assert_eq!(
            (true, Duration::from_secs(34)),
            retry_strategy.should_retry(1)
        );
        assert_eq!(
            (false, Duration::from_secs(34)),
            retry_strategy.should_retry(2)
        );
    }
}
