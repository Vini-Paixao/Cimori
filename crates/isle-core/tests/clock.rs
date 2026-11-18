#![allow(clippy::field_reassign_with_default)]
use isle_core::clock::{clock_text, date_text};
use isle_core::config::{DateOrder, IsleConfig};

/// 2026-11-11 22:41:07, local time (a Wednesday).
fn t() -> i64 {
    isle_core::agenda::days_from_civil(2026, 11, 11) * 86_400 + 22 * 3600 + 41 * 60 + 7
}

#[test]
fn defaults_read_as_today() {
    let c = IsleConfig::default();
    assert_eq!(clock_text(t(), &c), "22:41");
    assert_eq!(date_text(t(), &c), "qua 11 nov");
}

#[test]
fn clock_options() {
    let mut c = IsleConfig::default();
    c.clock_seconds = true;
    assert_eq!(clock_text(t(), &c), "22:41:07");
    c.clock_24h = false;
    c.clock_seconds = false;
    assert_eq!(clock_text(t(), &c), "10:41 PM");
    assert_eq!(clock_text(t() - 22 * 3600, &c), "12:41 AM");
}

#[test]
fn date_options() {
    let mut c = IsleConfig::default();
    c.clock_weekday = false;
    assert_eq!(date_text(t(), &c), "11 nov");
    c.month_number = true;
    assert_eq!(date_text(t(), &c), "11/11");
    c.date_order = DateOrder::MonthDay;
    c.month_number = false;
    assert_eq!(date_text(t(), &c), "nov 11");
    c.clock_year = true;
    assert_eq!(date_text(t(), &c), "nov 11 2026");
    c.clock_date = false;
    c.clock_weekday = true;
    assert_eq!(date_text(t(), &c), "qua");
}
