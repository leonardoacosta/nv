pub mod resend;
pub mod sentry;
pub mod stripe;
pub mod upstash;
pub mod vercel;

// ── Shared Formatting Helpers ────────────────────────────────────────

/// Convert an ISO 8601 / RFC 3339 timestamp string to a human-readable
/// relative time string for Telegram output.
///
/// - `"just now"` — less than 1 minute ago
/// - `"5m ago"` — less than 1 hour ago
/// - `"3h ago"` — less than 24 hours ago
/// - `"2d ago"` — less than 7 days ago
/// - `"Mar 15"` — 7 days or older (month abbreviation + day)
/// - `""` — if the timestamp cannot be parsed
pub fn relative_time(timestamp: &str) -> String {
    if timestamp.is_empty() {
        return String::new();
    }

    let epoch_secs = parse_iso8601_to_epoch(timestamp);
    let epoch_secs = match epoch_secs {
        Some(s) => s,
        None => return String::new(),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if now < epoch_secs {
        return "just now".to_string();
    }

    let diff = now - epoch_secs;

    if diff < 60 {
        return "just now".to_string();
    } else if diff < 3600 {
        return format!("{}m ago", diff / 60);
    } else if diff < 86400 {
        return format!("{}h ago", diff / 3600);
    } else if diff < 7 * 86400 {
        return format!("{}d ago", diff / 86400);
    }

    // Older than 7 days — parse out month+day for display ("Mar 15")
    month_day_from_iso(timestamp)
        .unwrap_or_else(|| timestamp.get(..10).unwrap_or(timestamp).to_string())
}

/// Parse an ISO 8601 date-time string to Unix seconds.
fn parse_iso8601_to_epoch(s: &str) -> Option<u64> {
    if s.len() < 10 {
        return None;
    }

    let date_part = s.get(..10)?;
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return None;
    }

    let year: i64 = parts[0].parse().ok()?;
    let month: i64 = parts[1].parse().ok()?;
    let day: i64 = parts[2].parse().ok()?;

    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (hour, minute, second) = if s.len() > 10 && s.as_bytes().get(10) == Some(&b'T') {
        let time = s.get(11..19).unwrap_or("00:00:00");
        let tp: Vec<&str> = time.split(':').collect();
        let h: i64 = tp.first().and_then(|v| v.parse().ok()).unwrap_or(0);
        let m: i64 = tp.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        let sc: i64 = tp.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
        (h, m, sc)
    } else {
        (0, 0, 0)
    };

    let days = days_since_epoch(year, month, day)?;
    let total_secs = days * 86400 + hour * 3600 + minute * 60 + second;

    let offset_secs = if s.len() > 19 {
        let tz = &s[19..];
        if tz.starts_with('+') || tz.starts_with('-') {
            let sign: i64 = if tz.starts_with('+') { 1 } else { -1 };
            let tz_digits = &tz[1..];
            let tp: Vec<&str> = tz_digits.split(':').collect();
            let oh: i64 = tp.first().and_then(|v| v.parse().ok()).unwrap_or(0);
            let om: i64 = tp.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            sign * (oh * 3600 + om * 60)
        } else {
            0
        }
    } else {
        0
    };

    let utc_secs = total_secs - offset_secs;
    if utc_secs < 0 {
        None
    } else {
        Some(utc_secs as u64)
    }
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    const MONTH_DAYS: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    if !(1..=12).contains(&month) {
        return None;
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let leap_day: i64 = if is_leap && month > 2 { 1 } else { 0 };

    let y = year - 1970;
    let leap_years = (y - 1) / 4 - (y - 1) / 100 + (y - 1) / 400
        + if year > 1972 { 1 } else { 0 };
    let year_days = y * 365 + leap_years;

    let month_idx = (month - 1) as usize;
    let total = year_days + MONTH_DAYS[month_idx] + leap_day + (day - 1);
    Some(total)
}

fn month_day_from_iso(s: &str) -> Option<String> {
    let date_part = s.get(..10)?;
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let month: usize = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;

    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month_name = MONTHS.get(month.wrapping_sub(1))?;
    Some(format!("{month_name} {day}"))
}
