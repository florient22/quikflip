use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

/// Knows what "today" is in the configured time zone; every daily puzzle is keyed by this date.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    timezone: Tz,
}

impl Clock {
    pub fn new(timezone: Tz) -> Self {
        Self { timezone }
    }

    pub fn today(&self) -> NaiveDate {
        day_at(Utc::now(), self.timezone)
    }

    pub fn seconds_until_next_day(&self) -> i64 {
        seconds_until_next_day(Utc::now(), self.timezone)
    }
}

pub fn day_at(now: DateTime<Utc>, timezone: Tz) -> NaiveDate {
    now.with_timezone(&timezone).date_naive()
}

pub fn seconds_until_next_day(now: DateTime<Utc>, timezone: Tz) -> i64 {
    let tomorrow = day_at(now, timezone) + Days::new(1);
    let midnight = tomorrow.and_hms_opt(0, 0, 0).expect("midnight is a valid time");
    // In zones where a DST change skips midnight, the day starts at the first valid moment after it.
    let next = timezone
        .from_local_datetime(&midnight)
        .earliest()
        .or_else(|| timezone.from_local_datetime(&(midnight + chrono::Duration::hours(1))).earliest())
        .map(|local| local.with_timezone(&Utc))
        .unwrap_or(now + chrono::Duration::hours(24));
    (next - now).num_seconds().max(0)
}

pub fn format_day(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_follows_the_local_time_zone() {
        let late_utc = Utc.with_ymd_and_hms(2026, 11, 8, 23, 30, 0).unwrap();
        // 23:30 UTC is already 00:30 the next day in Brussels (UTC+1 in November).
        assert_eq!(format_day(day_at(late_utc, chrono_tz::Europe::Brussels)), "2026-11-09");
        assert_eq!(seconds_until_next_day(late_utc, chrono_tz::Europe::Brussels), 23 * 3600 + 30 * 60);
    }
}
