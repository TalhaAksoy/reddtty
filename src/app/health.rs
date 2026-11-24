//! Provider/source health observed by the UI.
//!
//! This is deliberately in-memory: it answers "is the source working right
//! now?" and never uploads telemetry. The Source screen renders it.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::providers::ProviderStatus;

#[derive(Clone, Debug, Default)]
pub struct SourceHealth {
    pub last_status: Option<ProviderStatus>,
    pub last_error: Option<String>,
    pub last_attempt: Option<Instant>,
    pub last_success: Option<Instant>,
    pub last_latency: Option<Duration>,
    pub requests: u64,
    pub failures: u64,
    samples: VecDeque<Duration>,
}

impl SourceHealth {
    pub const SAMPLE_LIMIT: usize = 12;

    pub fn record(
        &mut self,
        status: ProviderStatus,
        error: Option<String>,
        latency: Option<Duration>,
    ) {
        let now = Instant::now();
        self.last_attempt = Some(now);
        self.requests = self.requests.saturating_add(1);
        self.last_status = Some(status);
        if status == ProviderStatus::Available {
            self.last_success = Some(now);
            self.last_error = None;
        } else {
            self.failures = self.failures.saturating_add(1);
            self.last_error = error;
        }
        if let Some(latency) = latency {
            self.last_latency = Some(latency);
            self.samples.push_back(latency);
            while self.samples.len() > Self::SAMPLE_LIMIT {
                self.samples.pop_front();
            }
        }
    }

    pub fn average_latency(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let total: Duration = self.samples.iter().sum();
        Some(total / self.samples.len() as u32)
    }

    pub fn success_rate(&self) -> Option<u8> {
        if self.requests == 0 {
            return None;
        }
        let successes = self.requests.saturating_sub(self.failures);
        Some(((successes * 100) / self.requests) as u8)
    }

    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success.map(|at| at.elapsed())
    }

    pub fn last_attempt_age(&self) -> Option<Duration> {
        self.last_attempt.map(|at| at.elapsed())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_degraded(&self) -> bool {
        self.last_status
            .is_some_and(|status| status != ProviderStatus::Available)
    }
}

pub fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1_000 {
        format!("{millis} ms")
    } else if millis < 60_000 {
        format!("{:.1} s", duration.as_secs_f64())
    } else {
        format!("{} min", duration.as_secs() / 60)
    }
}

pub fn format_age(duration: Option<Duration>) -> String {
    match duration {
        None => "never".into(),
        Some(duration) if duration.as_secs() < 5 => "just now".into(),
        Some(duration) if duration.as_secs() < 60 => format!("{}s ago", duration.as_secs()),
        Some(duration) if duration.as_secs() < 3_600 => {
            format!("{}m ago", duration.as_secs() / 60)
        }
        Some(duration) => format!("{}h ago", duration.as_secs() / 3_600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_success_and_failure_rates() {
        let mut health = SourceHealth::default();
        health.record(
            ProviderStatus::Available,
            None,
            Some(Duration::from_millis(200)),
        );
        health.record(
            ProviderStatus::RateLimited,
            Some("slow down".into()),
            Some(Duration::from_millis(400)),
        );
        assert_eq!(health.requests, 2);
        assert_eq!(health.failures, 1);
        assert_eq!(health.success_rate(), Some(50));
        assert_eq!(health.average_latency(), Some(Duration::from_millis(300)));
        assert_eq!(health.last_status, Some(ProviderStatus::RateLimited));
        assert_eq!(health.last_error.as_deref(), Some("slow down"));
        assert!(health.last_success.is_some());
        assert!(health.is_degraded());
    }

    #[test]
    fn success_clears_the_previous_error() {
        let mut health = SourceHealth::default();
        health.record(ProviderStatus::AccessDenied, Some("blocked".into()), None);
        health.record(ProviderStatus::Available, None, None);
        assert!(health.last_error.is_none());
        assert_eq!(health.success_rate(), Some(50));
        assert!(!health.is_degraded());
    }

    #[test]
    fn formatting_is_compact() {
        assert_eq!(format_duration(Duration::from_millis(12)), "12 ms");
        assert_eq!(format_duration(Duration::from_millis(1_500)), "1.5 s");
        assert_eq!(format_age(None), "never");
        assert_eq!(format_age(Some(Duration::from_secs(2))), "just now");
        assert_eq!(format_age(Some(Duration::from_secs(90))), "1m ago");
    }
}
