//! Decides what to do about the WiFi link.
//! This is useful because, if for some unforeseen reason the router goes off,
//! there has to be SOMETHING that manages the reconnection.
//! Well, this is the base for that "someting".

use core::time::Duration;

/// How long the link may stay down before the board restarts.
// TODO: Hardcoding this in the library seems like a bad idea. Will fix later.
pub const DOWN_LIMIT: Duration = Duration::from_mins(3);

/// What the caller should do after one check of the link.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The link is up and was already up: nothing to do
    Wait,
    /// First check of a drop: log a warning and start reconnecting
    Reconnect,
    /// Still down, we are still within `DOWN_LIMIT`
    Retry,
    /// The link came back after a drop, should log it.
    Recovered,
    /// Down for `DOWN_LIMIT` or longer, should restart the board
    Restart,
}

/// Remembers, between checks, when the link went down
#[derive(Debug, Default)]
pub struct LinkMonitor {
    /// Time since boot of the first check that found the link down,
    /// or `None` while the link is up
    down_since: Option<Duration>,
}

impl LinkMonitor {
    /// Decides what to do after one check of the link.
    pub fn step(&mut self, is_up: bool, now: Duration) -> Action {
        // my good friend claude made this table for me:
        // ┌───────┬───────────────────────────────┬───────────┬──────────────────┐
        // │ is_up │       down_since before       │  Result   │ down_since after │
        // ├───────┼───────────────────────────────┼───────────┼──────────────────┤
        // │ true  │ None                          │ Wait      │ None             │
        // ├───────┼───────────────────────────────┼───────────┼──────────────────┤
        // │ true  │ Some(_)                       │ Recovered │ None             │
        // ├───────┼───────────────────────────────┼───────────┼──────────────────┤
        // │ false │ None                          │ Reconnect │ Some(now)        │
        // ├───────┼───────────────────────────────┼───────────┼──────────────────┤
        // │ false │ Some(t), elapsed < DOWN_LIMIT │ Retry     │ unchanged        │
        // ├───────┼───────────────────────────────┼───────────┼──────────────────┤
        // │ false │ Some(t), elapsed ≥ DOWN_LIMIT │ Restart   │ unchanged        │
        // └───────┴───────────────────────────────┴───────────┴──────────────────┘
        match (is_up, self.down_since) {
            (true, None) => Action::Wait,
            (true, Some(_)) => {
                self.down_since = None;
                Action::Recovered
            }
            (false, None) => {
                self.down_since = Some(now);
                Action::Reconnect
            }
            (false, Some(since)) => {
                if now.saturating_sub(since) < DOWN_LIMIT {
                    Action::Retry
                } else {
                    Action::Restart
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A time `n` seconds after boot.
    fn to_secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn up_without_a_drop_waits() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(true, to_secs(0)), Action::Wait);
    }

    #[test]
    fn first_down_check_reconnects() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
    }

    #[test]
    fn second_down_check_retries() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        assert_eq!(monitor.step(false, to_secs(15)), Action::Retry);
    }

    #[test]
    fn down_just_under_the_limit_retries() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        let just_under = (to_secs(10) + DOWN_LIMIT)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert_eq!(monitor.step(false, just_under), Action::Retry);
    }

    #[test]
    fn down_at_the_limit_restarts() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        assert_eq!(
            monitor.step(false, to_secs(10) + DOWN_LIMIT),
            Action::Restart
        );
    }

    #[test]
    fn recovery_is_reported_once() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        assert_eq!(monitor.step(true, to_secs(20)), Action::Recovered);
        assert_eq!(monitor.step(true, to_secs(25)), Action::Wait);
    }

    #[test]
    fn drop_after_recovery_reconnects_and_restarts_the_timer() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        assert_eq!(monitor.step(true, to_secs(20)), Action::Recovered);
        assert_eq!(monitor.step(false, to_secs(30)), Action::Reconnect);
        let just_under = (to_secs(30) + DOWN_LIMIT)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert_eq!(monitor.step(false, just_under), Action::Retry);
    }

    #[test]
    fn clock_going_backwards_does_not_panic() {
        let mut monitor = LinkMonitor::default();
        assert_eq!(monitor.step(false, to_secs(10)), Action::Reconnect);
        assert_eq!(monitor.step(false, to_secs(5)), Action::Retry);
    }
}
