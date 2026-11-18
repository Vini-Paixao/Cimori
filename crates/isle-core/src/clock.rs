//! The bar's clock and date, as the settings say (Mac Island & Dock's clock options).
use crate::agenda::civil_from_days;
use crate::config::{DateOrder, IsleConfig};

pub const WEEKDAYS: [&str; 7] = ["qui", "sex", "sáb", "dom", "seg", "ter", "qua"]; // 1970-01-01 was a Thursday
const MONTHS: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];

/// `local_secs`: seconds since 1970 in local time.
pub fn clock_text(local_secs: i64, cfg: &IsleConfig) -> String {
    let s = local_secs.rem_euclid(86_400);
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    let (shown, suffix) = if cfg.clock_24h {
        (h, "")
    } else {
        ((h + 11) % 12 + 1, if h < 12 { " AM" } else { " PM" })
    };
    let hours = if cfg.clock_24h { format!("{shown:02}") } else { shown.to_string() };
    if cfg.clock_seconds {
        format!("{hours}:{m:02}:{sec:02}{suffix}")
    } else {
        format!("{hours}:{m:02}{suffix}")
    }
}

pub fn date_text(local_secs: i64, cfg: &IsleConfig) -> String {
    let days = local_secs.div_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let mut parts = Vec::new();
    if cfg.clock_weekday {
        parts.push(WEEKDAYS[days.rem_euclid(7) as usize].to_string());
    }
    if cfg.clock_date {
        let month = MONTHS[(mo - 1) as usize];
        let month_first = cfg.date_order == DateOrder::MonthDay;
        let date = match (cfg.month_number, month_first) {
            (true, false) => format!("{d:02}/{mo:02}"),
            (true, true) => format!("{mo:02}/{d:02}"),
            (false, false) => format!("{d} {month}"),
            (false, true) => format!("{month} {d}"),
        };
        parts.push(date);
        if cfg.clock_year {
            parts.push(y.to_string());
        }
    }
    parts.join(" ")
}
