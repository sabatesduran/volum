use std::time::SystemTime;

pub use jiff::{SignedDuration, Timestamp};

/// Formats a UTC timestamp as RFC 3339 with a `+00:00` offset and 0, 3, 6 or 9
/// fractional digits, matching the strings Volum has always stored.
pub fn rfc3339(value: Timestamp) -> String {
    let nanos = value.subsec_nanosecond().unsigned_abs();
    let fraction = if nanos == 0 {
        String::new()
    } else if nanos.is_multiple_of(1_000_000) {
        format!(".{:03}", nanos / 1_000_000)
    } else if nanos.is_multiple_of(1_000) {
        format!(".{:06}", nanos / 1_000)
    } else {
        format!(".{nanos:09}")
    };
    format!("{}{fraction}+00:00", value.strftime("%Y-%m-%dT%H:%M:%S"))
}

pub fn now_rfc3339() -> String {
    rfc3339(Timestamp::now())
}

pub fn parse_rfc3339(value: &str) -> Option<Timestamp> {
    value.parse().ok()
}

pub fn from_system_time(value: SystemTime) -> Timestamp {
    Timestamp::try_from(value).unwrap_or_else(|_| Timestamp::now())
}

/// `YYYYMMDDTHHMMSSZ`, for file names.
pub fn compact(value: Timestamp) -> String {
    value.strftime("%Y%m%dT%H%M%SZ").to_string()
}

pub fn is_calendar_date(value: &str) -> bool {
    jiff::civil::Date::strptime("%Y-%m-%d", value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_stored_timestamps() {
        let at = |value: &str| rfc3339(parse_rfc3339(value).unwrap());
        assert_eq!(at("2026-09-21T12:00:01Z"), "2026-09-21T12:00:01+00:00");
        assert_eq!(
            at("2026-09-21T12:00:01.5Z"),
            "2026-09-21T12:00:01.500+00:00"
        );
        assert_eq!(
            at("2026-09-21T12:00:01.000250Z"),
            "2026-09-21T12:00:01.000250+00:00"
        );
        assert_eq!(
            at("2026-09-21T14:00:01.123456789+02:00"),
            "2026-09-21T12:00:01.123456789+00:00"
        );
        assert_eq!(
            compact(parse_rfc3339("2026-09-21T12:00:01.9Z").unwrap()),
            "20260921T120001Z"
        );
    }

    #[test]
    fn validates_calendar_dates() {
        assert!(is_calendar_date("2028-02-29"));
        assert!(!is_calendar_date("2027-02-29"));
        assert!(!is_calendar_date("2027-13-01"));
        assert!(!is_calendar_date("not a date"));
    }
}
