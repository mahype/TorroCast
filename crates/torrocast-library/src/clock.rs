//! Hybrid logical clocks: wall time where it can be trusted, a counter where
//! it cannot, and the device as the last word — so any two changes, from any
//! two devices, have an order, and every device agrees on it.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, SecondsFormat, Utc};

/// A remote clock further ahead than this is wrong, not early. Its changes
/// are still applied, but our own clock does not follow it.
const TRUSTED_LEAD_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc {
    pub ms: u64,
    pub counter: u16,
    pub device: String,
}

impl fmt::Display for Hlc {
    /// `2026-09-19T10:02:11.120Z-0000-7f3a9c2e`: readable, and sorting the
    /// strings sorts the clocks.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let moment = DateTime::<Utc>::from_timestamp_millis(self.ms as i64).unwrap_or_default();
        write!(
            formatter,
            "{}-{:04x}-{}",
            moment.to_rfc3339_opts(SecondsFormat::Millis, true),
            self.counter,
            self.device
        )
    }
}

impl FromStr for Hlc {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // The timestamp itself contains dashes; split from the right.
        let mut parts = text.rsplitn(3, '-');
        let (device, counter, moment) = (parts.next().ok_or(())?, parts.next().ok_or(())?, parts.next().ok_or(())?);
        let moment = DateTime::parse_from_rfc3339(moment).map_err(|_| ())?;
        Ok(Self {
            ms: u64::try_from(moment.timestamp_millis()).map_err(|_| ())?,
            counter: u16::from_str_radix(counter, 16).map_err(|_| ())?,
            device: device.to_owned(),
        })
    }
}

pub struct Clock {
    device: String,
    ms: u64,
    counter: u16,
}

impl Clock {
    #[must_use]
    pub fn new(device: &str) -> Self {
        Self { device: device.to_owned(), ms: 0, counter: 0 }
    }

    /// The stamp for a new change. Never goes backwards, whatever the wall clock does.
    pub fn tick(&mut self, now_ms: u64) -> Hlc {
        if now_ms > self.ms {
            (self.ms, self.counter) = (now_ms, 0);
        } else {
            self.counter = self.counter.saturating_add(1);
        }
        Hlc { ms: self.ms, counter: self.counter, device: self.device.clone() }
    }

    /// Takes note of a stamp seen elsewhere, so our next change is later than it.
    pub fn observe(&mut self, seen: &Hlc, now_ms: u64) {
        if seen.ms > now_ms.saturating_add(TRUSTED_LEAD_MS) {
            return;
        }
        if (seen.ms, seen.counter) > (self.ms, self.counter) {
            (self.ms, self.counter) = (seen.ms, seen.counter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Hlc};

    #[test]
    fn strings_sort_like_clocks() {
        let early = Hlc { ms: 1_789_000_000_000, counter: 9, device: "zz".into() };
        let late = Hlc { ms: 1_789_000_000_001, counter: 0, device: "aa".into() };
        assert!(early < late);
        assert!(early.to_string() < late.to_string());
        assert_eq!(early.to_string().parse::<Hlc>(), Ok(early));
        assert_eq!(
            "2026-09-19T10:02:11.120Z-000a-7f3a9c2e".parse::<Hlc>().map(|hlc| (hlc.counter, hlc.device)),
            Ok((10, "7f3a9c2e".into()))
        );
    }

    #[test]
    fn never_backwards_even_if_the_wall_clock_is() {
        let mut clock = Clock::new("a");
        let first = clock.tick(5_000);
        let second = clock.tick(4_000);
        let third = clock.tick(4_000);
        assert!(first < second && second < third);
    }

    #[test]
    fn a_clock_from_the_far_future_is_not_followed() {
        let mut clock = Clock::new("a");
        let now = 1_789_000_000_000;
        clock.observe(&Hlc { ms: now + 60_000, counter: 3, device: "b".into() }, now);
        assert!(clock.tick(now).ms == now + 60_000, "a minute ahead is ordinary drift");
        clock.observe(&Hlc { ms: now + 10 * 365 * 86_400_000, counter: 0, device: "c".into() }, now);
        assert!(clock.tick(now).ms < now + 120_000, "ten years ahead is a broken clock");
    }
}
