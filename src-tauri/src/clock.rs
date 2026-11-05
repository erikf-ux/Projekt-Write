//! Time stamps and ids (section 18: "IDs als UUID-Text, Zeiten in UTC nach
//! ISO 8601").
//!
//! Both come from [`Generators`] so that tests and the reproducible
//! reference library (`scripts/reference-library.mjs`) can replace the system
//! clock and the random ids with deterministic sources.

use std::fmt;
use std::sync::Arc;

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
use uuid::Uuid;

/// An instant in UTC with millisecond precision.
///
/// Its text form is always `YYYY-MM-DDTHH:MM:SS.mmmZ`. The fixed width keeps
/// the lexical order of stored values equal to their chronological order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Earliest supported instant: 0001-01-01T00:00:00.000Z.
    const MIN_MILLIS: i64 = -62_135_596_800_000;
    /// Latest supported instant: 9999-12-31T23:59:59.999Z.
    const MAX_MILLIS: i64 = 253_402_300_799_999;

    /// From milliseconds since the Unix epoch. `None` outside years 1–9999.
    pub fn from_unix_millis(millis: i64) -> Option<Self> {
        (Self::MIN_MILLIS..=Self::MAX_MILLIS)
            .contains(&millis)
            .then_some(Self(millis))
    }

    pub fn unix_millis(self) -> i64 {
        self.0
    }

    /// The current system time.
    pub fn now() -> Self {
        let now = OffsetDateTime::now_utc();
        let millis = now.unix_timestamp() * 1000 + i64::from(now.millisecond());
        Self(millis.clamp(Self::MIN_MILLIS, Self::MAX_MILLIS))
    }

    fn date_time(self) -> OffsetDateTime {
        let nanos = i128::from(self.0) * 1_000_000;
        OffsetDateTime::from_unix_timestamp_nanos(nanos).expect("range checked in from_unix_millis")
    }

    /// `YYYY-MM-DDTHH:MM:SS.mmmZ`.
    pub fn to_iso(self) -> String {
        let dt = self.date_time();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            dt.year(),
            u8::from(dt.month()),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.millisecond()
        )
    }

    /// `YYYYMMDDTHHMMSSmmmZ`, for file names.
    pub fn to_compact(self) -> String {
        let dt = self.date_time();
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}{:03}Z",
            dt.year(),
            u8::from(dt.month()),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.millisecond()
        )
    }

    /// Parses `YYYY-MM-DDTHH:MM:SSZ` with an optional fraction of 1–9 digits
    /// (`.123Z`). Precision beyond milliseconds is truncated. Anything else,
    /// including offsets other than `Z`, is rejected.
    pub fn parse_iso(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() < 20 || bytes.last() != Some(&b'Z') {
            return None;
        }
        let digits = |range: std::ops::Range<usize>| -> Option<u32> {
            let part = bytes.get(range)?;
            if !part.iter().all(u8::is_ascii_digit) {
                return None;
            }
            std::str::from_utf8(part).ok()?.parse().ok()
        };
        let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
        if separators.iter().any(|&(i, c)| bytes[i] != c) {
            return None;
        }
        let year = i32::try_from(digits(0..4)?).ok()?;
        let month = Month::try_from(u8::try_from(digits(5..7)?).ok()?).ok()?;
        let day = u8::try_from(digits(8..10)?).ok()?;
        let hour = u8::try_from(digits(11..13)?).ok()?;
        let minute = u8::try_from(digits(14..16)?).ok()?;
        let second = u8::try_from(digits(17..19)?).ok()?;

        let rest = &bytes[19..bytes.len() - 1];
        let millis = match rest {
            [] => 0,
            [b'.', fraction @ ..] if (1..=9).contains(&fraction.len()) => {
                if !fraction.iter().all(u8::is_ascii_digit) {
                    return None;
                }
                fraction
                    .iter()
                    .chain(std::iter::repeat(&b'0'))
                    .take(3)
                    .fold(0u16, |acc, d| acc * 10 + u16::from(d - b'0'))
            }
            _ => return None,
        };

        let date = Date::from_calendar_date(year, month, day).ok()?;
        let time = Time::from_hms_milli(hour, minute, second, millis).ok()?;
        let dt = PrimitiveDateTime::new(date, time).assume_offset(UtcOffset::UTC);
        let millis = dt.unix_timestamp() * 1000 + i64::from(dt.millisecond());
        Self::from_unix_millis(millis)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso())
    }
}

/// Source of the current time.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Source of new ids.
pub trait IdSource: Send + Sync {
    fn new_id(&self) -> Uuid;
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// A clock that stands still until it is moved: for tests with simulated
/// time (retention of the trash and of the history, ADR 0016).
#[derive(Debug)]
pub struct ManualClock {
    millis: std::sync::atomic::AtomicI64,
}

impl ManualClock {
    pub fn new(start: Timestamp) -> Self {
        Self {
            millis: std::sync::atomic::AtomicI64::new(start.unix_millis()),
        }
    }

    /// Moves the clock forward (or back, with a negative value).
    pub fn advance(&self, millis: i64) {
        self.millis
            .fetch_add(millis, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set(&self, time: Timestamp) {
        self.millis
            .store(time.unix_millis(), std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_unix_millis(self.millis.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap_or_else(Timestamp::now)
    }
}

/// Random version 4 UUIDs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomIds;

impl IdSource for RandomIds {
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

/// Clock and id source used by the services.
#[derive(Clone)]
pub struct Generators {
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdSource>,
}

impl Generators {
    pub fn new(clock: Arc<dyn Clock>, ids: Arc<dyn IdSource>) -> Self {
        Self { clock, ids }
    }

    /// System clock and random ids.
    pub fn system() -> Self {
        Self::new(Arc::new(SystemClock), Arc::new(RandomIds))
    }

    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    pub fn new_id(&self) -> Uuid {
        self.ids.new_id()
    }
}

impl fmt::Debug for Generators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Generators")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_fixed_width() {
        let ts = Timestamp::from_unix_millis(1_793_786_400_007).unwrap();
        assert_eq!(ts.to_iso(), "2026-11-04T10:00:00.007Z");
        assert_eq!(ts.to_compact(), "20261104T100000007Z");
        assert_eq!(
            Timestamp::from_unix_millis(0).unwrap().to_iso(),
            "1970-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn parses_what_it_formats_and_common_variants() {
        for millis in [0, 1, 999, 1_793_786_400_007, -1, -62_135_596_800_000] {
            let ts = Timestamp::from_unix_millis(millis).unwrap();
            assert_eq!(Timestamp::parse_iso(&ts.to_iso()), Some(ts), "{millis}");
        }
        let base = Timestamp::parse_iso("2026-11-02T10:00:00Z").unwrap();
        assert_eq!(base.to_iso(), "2026-11-02T10:00:00.000Z");
        assert_eq!(
            Timestamp::parse_iso("2026-11-02T10:00:00.5Z")
                .unwrap()
                .unix_millis(),
            base.unix_millis() + 500
        );
        assert_eq!(
            Timestamp::parse_iso("2026-11-02T10:00:00.123456789Z")
                .unwrap()
                .unix_millis(),
            base.unix_millis() + 123
        );
    }

    #[test]
    fn rejects_malformed_and_non_utc_values() {
        for text in [
            "",
            "2026-11-02",
            "2026-11-02T10:00:00",
            "2026-11-02T10:00:00+01:00",
            "2026-11-02 10:00:00Z",
            "2026-13-02T10:00:00Z",
            "2026-02-30T10:00:00Z",
            "2026-11-02T24:00:00Z",
            "2026-11-02T10:60:00Z",
            "2026-11-02T10:00:00.Z",
            "2026-11-02T10:00:00.1234567890Z",
            "2026-11-02T10:00:00.12aZ",
            "+026-11-02T10:00:00Z",
            "２026-11-02T10:00:00Z",
        ] {
            assert_eq!(Timestamp::parse_iso(text), None, "{text:?}");
        }
    }

    #[test]
    fn range_is_limited_to_four_digit_years() {
        assert!(Timestamp::from_unix_millis(Timestamp::MAX_MILLIS).is_some());
        assert!(Timestamp::from_unix_millis(Timestamp::MAX_MILLIS + 1).is_none());
        assert!(Timestamp::from_unix_millis(Timestamp::MIN_MILLIS - 1).is_none());
        assert_eq!(
            Timestamp::from_unix_millis(Timestamp::MAX_MILLIS)
                .unwrap()
                .to_iso(),
            "9999-12-31T23:59:59.999Z"
        );
    }

    #[test]
    fn now_is_parseable() {
        let now = Timestamp::now();
        assert_eq!(Timestamp::parse_iso(&now.to_iso()), Some(now));
    }
}
