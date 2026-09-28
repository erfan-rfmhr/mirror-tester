//! Environment-driven runtime configuration.
//!
//! All values fall back to sensible defaults when the corresponding
//! environment variable is unset, empty, non-numeric, or not positive.

use std::env;

/// Defaults used when no environment override is provided.
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_ATTEMPTS: u32 = 1;
pub const DEFAULT_SCHEDULE_INTERVAL_SECS: u64 = 60 * 60;

/// Resolved runtime configuration for benchmarking and scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Per-attempt HTTP timeout in seconds.
    pub timeout_secs: u64,
    /// Number of benchmark attempts per mirror.
    pub attempts: u32,
    /// Delay between scheduler cycles in seconds.
    pub schedule_interval_secs: u64,
}

impl Config {
    /// Reads configuration from the environment, applying defaults for any
    /// missing or invalid value.
    pub fn from_env() -> Self {
        Config {
            timeout_secs: positive_var("AYN_TIMEOUT", DEFAULT_TIMEOUT_SECS),
            attempts: positive_var("AYN_ATTEMPTS", DEFAULT_ATTEMPTS),
            schedule_interval_secs: positive_var(
                "AYN_SCHEDULE_INTERVAL_SECS",
                DEFAULT_SCHEDULE_INTERVAL_SECS,
            ),
        }
    }

    fn defaults() -> Self {
        Config {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            attempts: DEFAULT_ATTEMPTS,
            schedule_interval_secs: DEFAULT_SCHEDULE_INTERVAL_SECS,
        }
    }
}

/// Types accepted as a positive integer environment override.
trait Positive: Sized {
    const ZERO: Self;
}

impl Positive for u64 {
    const ZERO: Self = 0;
}

impl Positive for u32 {
    const ZERO: Self = 0;
}

/// Parses an environment variable as a positive integer, returning `default`
/// when it is unset, empty, non-numeric, zero, or negative.
fn positive_var<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + PartialOrd + Copy + Positive,
{
    match env::var(key) {
        Ok(raw) => match raw.trim().parse::<T>() {
            Ok(value) if value > T::ZERO => value,
            _ => default,
        },
        Err(_) => default,
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::env;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVar {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvVar {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var_os(key);
            unsafe { env::set_var(key, value) };
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = env::var_os(key);
            unsafe { env::remove_var(key) };
            Self { key, previous }
        }
    }

    impl Drop for EnvVar {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { env::set_var(self.key, value) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn timeout_defaults_when_unset() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::unset("AYN_TIMEOUT");
        assert_eq!(Config::from_env().timeout_secs, 15);
    }

    #[test]
    fn timeout_uses_env_when_valid() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_TIMEOUT", "42");
        assert_eq!(Config::from_env().timeout_secs, 42);
    }

    #[test]
    fn timeout_falls_back_when_non_numeric() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_TIMEOUT", "fast");
        assert_eq!(Config::from_env().timeout_secs, 15);
    }

    #[test]
    fn timeout_falls_back_when_zero() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_TIMEOUT", "0");
        assert_eq!(Config::from_env().timeout_secs, 15);
    }

    #[test]
    fn attempts_defaults_when_unset() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::unset("AYN_ATTEMPTS");
        assert_eq!(Config::from_env().attempts, 3);
    }

    #[test]
    fn attempts_uses_env_when_valid() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_ATTEMPTS", "5");
        assert_eq!(Config::from_env().attempts, 5);
    }

    #[test]
    fn attempts_falls_back_when_non_numeric() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_ATTEMPTS", "many");
        assert_eq!(Config::from_env().attempts, 3);
    }

    #[test]
    fn attempts_falls_back_when_zero() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_ATTEMPTS", "0");
        assert_eq!(Config::from_env().attempts, 3);
    }

    #[test]
    fn interval_defaults_when_unset() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::unset("AYN_SCHEDULE_INTERVAL_SECS");
        assert_eq!(Config::from_env().schedule_interval_secs, 3600);
    }

    #[test]
    fn interval_uses_env_when_valid() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_SCHEDULE_INTERVAL_SECS", "120");
        assert_eq!(Config::from_env().schedule_interval_secs, 120);
    }

    #[test]
    fn interval_falls_back_when_non_numeric() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_SCHEDULE_INTERVAL_SECS", "hourly");
        assert_eq!(Config::from_env().schedule_interval_secs, 3600);
    }

    #[test]
    fn interval_falls_back_when_zero() {
        let _lock = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _env = EnvVar::set("AYN_SCHEDULE_INTERVAL_SECS", "0");
        assert_eq!(Config::from_env().schedule_interval_secs, 3600);
    }
}
