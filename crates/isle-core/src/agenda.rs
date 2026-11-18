// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (ParseIcs, IcsRule, ParseHolidays):
// .ics agendas with repeats, exceptions and moved occurrences, and Nager.Date holidays.

use serde::{Deserialize, Serialize};

/// One thing on one local day: an event's occurrence or a holiday.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaItem {
    pub title: String,
    /// Local days since 1970-01-01.
    pub day: i64,
    /// Local minute of the day; `None` all day.
    pub minute: Option<i32>,
    pub holiday: bool,
}

/// What the island understands of a repeat (RRULE), as the mod's IcsRule.
#[derive(Debug, Clone, Default)]
struct Rule {
    /// 1 daily, 2 weekly, 3 monthly, 4 yearly.
    freq: u8,
    interval: i64,
    count: Option<u32>,
    /// Last local day, inclusive.
    until: Option<i64>,
    /// (which one in the month, 0 for each; weekday, 0 Sunday).
    by_day: Vec<(i32, i64)>,
    by_month_day: Vec<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct IcsEvent {
    uid: String,
    title: String,
    day: i64,
    minute: Option<i32>,
    rule: Option<Rule>,
    except: Vec<i64>,
    /// A moved occurrence: the day it replaces in its series (same UID).
    replaces: Option<i64>,
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// 0 Sunday … 6 Saturday.
pub fn weekday(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

pub fn month_len(y: i64, m: i64) -> i64 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)
}

/// "20261112T130000Z" (UTC, made local by `offset`), "20261116T083000" (zoned or floating: taken as
/// local) or "20261115" (all day). shortcut: a TZID other than the user's own is read as local; convert
/// through VTIMEZONE when agendas from other zones appear.
fn parse_time(value: &str, offset: i64) -> Option<(i64, Option<i32>)> {
    let v = value.trim();
    let num = |a: usize, b: usize| v.get(a..b)?.parse::<i64>().ok();
    let day = days_from_civil(num(0, 4)?, num(4, 6)?, num(6, 8)?);
    if v.len() < 15 || v.as_bytes()[8] != b'T' {
        return Some((day, None));
    }
    let mut secs = day * 86_400 + num(9, 11)? * 3600 + num(11, 13)? * 60;
    if v.ends_with('Z') {
        secs += offset;
    }
    Some((secs.div_euclid(86_400), Some((secs.rem_euclid(86_400) / 60) as i32)))
}

fn parse_rule(text: &str, offset: i64) -> Option<Rule> {
    let mut rule = Rule { interval: 1, ..Default::default() };
    for part in text.split(';') {
        let Some((key, value)) = part.split_once('=') else { continue };
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => rule.freq = match value.to_ascii_uppercase().as_str() {
                "DAILY" => 1, "WEEKLY" => 2, "MONTHLY" => 3, "YEARLY" => 4, _ => 0,
            },
            "INTERVAL" => rule.interval = value.parse::<i64>().unwrap_or(1).clamp(1, 1000),
            "COUNT" => rule.count = value.parse::<u32>().ok().map(|c| c.min(100_000)),
            "UNTIL" => rule.until = parse_time(value, offset).map(|(d, _)| d),
            "BYDAY" => {
                for item in value.split(',').take(64) {
                    let item = item.trim();
                    let (n, wd) = item.split_at(item.len().saturating_sub(2));
                    let wd = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"].iter().position(|w| w.eq_ignore_ascii_case(wd));
                    if let Some(wd) = wd {
                        rule.by_day.push((n.parse().unwrap_or(0), wd as i64));
                    }
                }
            }
            "BYMONTHDAY" => rule.by_month_day = value.split(',').take(62).filter_map(|d| d.parse().ok()).collect(),
            _ => {}
        }
    }
    (rule.freq != 0).then_some(rule)
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// `offset`: the user's local time minus UTC, in seconds.
pub fn parse_ics(raw: &str, offset: i64) -> Vec<IcsEvent> {
    // Unfold: a line starting with a space or tab continues the one before.
    let mut lines: Vec<String> = Vec::new();
    for line in raw.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(line.to_string()),
        }
    }
    let mut events = Vec::new();
    let mut current: Option<(IcsEvent, bool)> = None;
    for line in &lines {
        let Some((head, value)) = line.split_once(':') else { continue };
        let name = head.split(';').next().unwrap_or("").to_ascii_uppercase();
        match (name.as_str(), &mut current) {
            ("BEGIN", None) if value.eq_ignore_ascii_case("VEVENT") => current = Some((IcsEvent { day: i64::MIN, ..Default::default() }, false)),
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => {
                if let Some((event, cancelled)) = current.take()
                    && !cancelled && event.day != i64::MIN && events.len() < 5000
                {
                    events.push(event);
                }
            }
            ("UID", Some((e, _))) => e.uid = value.to_string(),
            ("SUMMARY", Some((e, _))) => e.title = unescape(value).chars().take(200).collect(),
            ("STATUS", Some((_, cancelled))) => *cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            ("DTSTART", Some((e, _))) => if let Some((day, minute)) = parse_time(value, offset) { e.day = day; e.minute = minute; },
            ("RRULE", Some((e, _))) => e.rule = parse_rule(value, offset),
            ("EXDATE", Some((e, _))) => e.except.extend(value.split(',').take(400).filter_map(|v| parse_time(v, offset)).map(|(d, _)| d)),
            ("RECURRENCE-ID", Some((e, _))) => e.replaces = parse_time(value, offset).map(|(d, _)| d),
            _ => {}
        }
    }
    events
}

/// The days of a repeat from its first, in order, until `last` (inclusive) or its end.
fn repeat_days(start: i64, rule: &Rule, last: i64) -> Vec<i64> {
    let last = rule.until.map_or(last, |u| u.min(last));
    let mut found = Vec::new();
    let mut taken = 0u32;
    let (sy, sm, sd) = civil_from_days(start);
    // shortcut: enough periods for any agenda's window; a malformed file stops here, not in a loop.
    for k in 0..20_000i64 {
        let step = k * rule.interval;
        let mut days: Vec<i64> = match rule.freq {
            1 => vec![start + step],
            2 => {
                let week = start - weekday(start) + 7 * step;
                let mut wds: Vec<i64> = rule.by_day.iter().map(|&(_, wd)| wd).collect();
                if wds.is_empty() { wds.push(weekday(start)); }
                wds.iter().map(|wd| week + wd).collect()
            }
            3 => {
                let index = sm - 1 + step;
                let (y, m) = (sy + index.div_euclid(12), index.rem_euclid(12) + 1);
                let len = month_len(y, m);
                let first = days_from_civil(y, m, 1);
                if !rule.by_day.is_empty() {
                    rule.by_day.iter().filter_map(|&(n, wd)| {
                        let all: Vec<i64> = (0..len).map(|i| first + i).filter(|&d| weekday(d) == wd).collect();
                        match n { 0 => None, n if n > 0 => all.get(n as usize - 1).copied(), n => all.len().checked_sub(n.unsigned_abs() as usize).and_then(|i| all.get(i).copied()) }
                            .map(|d| vec![d]).or_else(|| (n == 0).then_some(all))
                    }).flatten().collect()
                } else {
                    let wanted = if rule.by_month_day.is_empty() { vec![sd as i32] } else { rule.by_month_day.clone() };
                    wanted.iter().filter_map(|&d| {
                        let d = if d < 0 { len + 1 + d as i64 } else { d as i64 };
                        (1..=len).contains(&d).then(|| first + d - 1)
                    }).collect()
                }
            }
            4 => {
                let y = sy + step;
                (sd <= month_len(y, sm)).then(|| days_from_civil(y, sm, sd)).into_iter().collect()
            }
            _ => return found,
        };
        days.sort_unstable();
        days.dedup();
        for d in days {
            if d < start { continue; }
            if d > last || rule.count.is_some_and(|c| taken >= c) { return found; }
            taken += 1;
            found.push(d);
        }
    }
    found
}

/// Every occurrence on days `from..to`, by day then time (all-day first).
pub fn expand(events: &[IcsEvent], from: i64, to: i64) -> Vec<AgendaItem> {
    let mut items = Vec::new();
    for event in events {
        let days = match (&event.rule, event.replaces) {
            (Some(rule), None) => {
                let moved: Vec<i64> = events.iter()
                    .filter(|o| o.replaces.is_some() && o.uid == event.uid)
                    .filter_map(|o| o.replaces).collect();
                repeat_days(event.day, rule, to - 1).into_iter()
                    .filter(|d| !event.except.contains(d) && !moved.contains(d)).collect()
            }
            _ => vec![event.day],
        };
        items.extend(days.into_iter().filter(|d| (from..to).contains(d)).map(|day| AgendaItem {
            title: event.title.clone(), day, minute: event.minute, holiday: false,
        }));
    }
    items.sort_by_key(|i| (i.day, i.minute.map_or(-1, |m| m as i64)));
    items
}

/// Nager.Date's /api/v3/PublicHolidays/{year}/{country}, by their local names.
pub fn parse_holidays(json: &str) -> Vec<AgendaItem> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str(json) else { return Vec::new() };
    list.iter().take(100).filter_map(|h| {
        let date = h.get("date")?.as_str()?;
        let num = |a: usize, b: usize| date.get(a..b)?.parse::<i64>().ok();
        let title = h.get("localName").or_else(|| h.get("name"))?.as_str()?;
        Some(AgendaItem { title: title.chars().take(120).collect(), day: days_from_civil(num(0, 4)?, num(5, 7)?, num(8, 10)?), minute: None, holiday: true })
    }).collect()
}

/// The mod's ReminderDue: a reminder seen up to 2 minutes late is on time; one the island missed
/// (closed, asleep) still shows up to 12 hours late; older ones are dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderDue { Wait, OnTime, Missed, TooOld }

pub fn classify_reminder(late_minutes: i64) -> ReminderDue {
    match late_minutes {
        ..0 => ReminderDue::Wait,
        0..=2 => ReminderDue::OnTime,
        3..=720 => ReminderDue::Missed,
        _ => ReminderDue::TooOld,
    }
}

/// The key that marks an event's reminder as given (title, day and minute).
pub fn reminder_key(item: &AgendaItem) -> String {
    format!("{}|{}|{}", item.day, item.minute.unwrap_or(-1), item.title)
}

/// The timed events (not holidays, not all day) whose reminder, `lead` minutes before them, is
/// due at `now` (local minutes since 1970) and not given yet: each with whether it was missed.
/// Ones too old come back too (to be marked given without a banner).
pub fn due_reminders<'a>(items: &'a [AgendaItem], now: i64, lead: i64, given: &std::collections::HashSet<String>)
    -> Vec<(&'a AgendaItem, ReminderDue)> {
    items.iter()
        .filter(|i| !i.holiday && !given.contains(&reminder_key(i)))
        .filter_map(|i| {
            let due = classify_reminder(now - (i.day * 1440 + i.minute? as i64 - lead));
            (due != ReminderDue::Wait).then_some((i, due))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agenda_reminders_follow_the_mods_on_time_missed_and_too_old() {
        let event = |title: &str, minute: Option<i32>| AgendaItem { title: title.into(), day: 100, minute, holiday: false };
        let items = [event("Daily", Some(600)), event("Feriado", None), event("Almoço", Some(720)), event("Ontem", Some(0))];
        let mut given = std::collections::HashSet::new();
        let at = |m: i64| 100 * 1440 + m;
        // 10:01: the daily is on time; lunch (12:00) waits; all day never reminds.
        let due = due_reminders(&items, at(601), 0, &given);
        assert_eq!(due.iter().map(|(i, d)| (i.title.as_str(), *d)).collect::<Vec<_>>(), [("Daily", ReminderDue::OnTime), ("Ontem", ReminderDue::Missed)]);
        given.insert(reminder_key(&items[0]));
        // Ten minutes before lunch, with a 10-minute lead.
        let due = due_reminders(&items, at(710), 10, &given);
        assert!(due.iter().any(|(i, d)| i.title == "Almoço" && *d == ReminderDue::OnTime));
        assert!(!due.iter().any(|(i, _)| i.title == "Daily"));
        assert_eq!(classify_reminder(721), ReminderDue::TooOld);
    }

    const OFFSET: i64 = -3 * 3600;

    fn ics(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\n{body}END:VCALENDAR\r\n")
    }
    fn titles_on(items: &[AgendaItem], y: i64, m: i64, d: i64) -> Vec<String> {
        let day = days_from_civil(y, m, d);
        items.iter().filter(|i| i.day == day).map(|i| i.title.clone()).collect()
    }

    #[test]
    fn utc_times_become_local_and_folded_lines_join() {
        let raw = ics("BEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261112T130000Z\r\nDTEND:20261112T140000Z\r\nSUMMARY:Reuni\r\n ão do time\\, sala 2\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 11, 1), days_from_civil(2026, 12, 1));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Reunião do time, sala 2");
        assert_eq!(items[0].minute, Some(10 * 60)); // 13:00Z is 10:00 in São Paulo
        assert_eq!(items[0].day, days_from_civil(2026, 11, 12));
    }

    #[test]
    fn all_day_and_zoned_times_stay_on_their_day() {
        let raw = ics("BEGIN:VEVENT\r\nUID:b\r\nDTSTART;VALUE=DATE:20261115\r\nSUMMARY:Viagem\r\nEND:VEVENT\r\n\
            BEGIN:VEVENT\r\nUID:c\r\nDTSTART;TZID=America/Sao_Paulo:20261116T083000\r\nSUMMARY:Dentista\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 11, 1), days_from_civil(2026, 12, 1));
        assert_eq!(titles_on(&items, 2026, 11, 15), vec!["Viagem"]);
        assert_eq!(items.iter().find(|i| i.title == "Viagem").unwrap().minute, None);
        assert_eq!(items.iter().find(|i| i.title == "Dentista").unwrap().minute, Some(8 * 60 + 30));
    }

    #[test]
    fn monthly_by_month_day_skips_months_without_that_day_and_stops_at_until() {
        let raw = ics("BEGIN:VEVENT\r\nUID:d\r\nDTSTART;VALUE=DATE:20260130\r\nRRULE:FREQ=MONTHLY;WKST=MO;UNTIL=20260501;BYMONTHDAY=30\r\nSUMMARY:Fatura\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 1, 1), days_from_civil(2026, 12, 31));
        let days: Vec<_> = items.iter().map(|i| civil_from_days(i.day)).collect();
        assert_eq!(days, vec![(2026, 1, 30), (2026, 3, 30), (2026, 4, 30)]);
    }

    #[test]
    fn daily_interval_count_and_exdate() {
        let raw = ics("BEGIN:VEVENT\r\nUID:e\r\nDTSTART;VALUE=DATE:20261101\r\nRRULE:FREQ=DAILY;INTERVAL=14;COUNT=4\r\nEXDATE;VALUE=DATE:20261115\r\nSUMMARY:Plantão\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 10, 1), days_from_civil(2027, 3, 1));
        let days: Vec<_> = items.iter().map(|i| civil_from_days(i.day)).collect();
        assert_eq!(days, vec![(2026, 11, 1), (2026, 11, 29), (2026, 12, 13)]);
    }

    #[test]
    fn weekly_by_day_and_yearly() {
        let raw = ics("BEGIN:VEVENT\r\nUID:f\r\nDTSTART:20261102T120000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4\r\nSUMMARY:Academia\r\nEND:VEVENT\r\n\
            BEGIN:VEVENT\r\nUID:g\r\nDTSTART;VALUE=DATE:20200310\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Aniversário\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 1, 1), days_from_civil(2027, 1, 1));
        let gym: Vec<_> = items.iter().filter(|i| i.title == "Academia").map(|i| civil_from_days(i.day)).collect();
        assert_eq!(gym, vec![(2026, 11, 2), (2026, 11, 4), (2026, 11, 9), (2026, 11, 11)]);
        assert_eq!(titles_on(&items, 2026, 3, 10), vec!["Aniversário"]);
    }

    #[test]
    fn a_moved_occurrence_replaces_the_repeat_on_its_original_day() {
        let raw = ics("BEGIN:VEVENT\r\nUID:h\r\nDTSTART;VALUE=DATE:20261104\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=4\r\nSUMMARY:Aluguel\r\nEND:VEVENT\r\n\
            BEGIN:VEVENT\r\nUID:h\r\nRECURRENCE-ID;VALUE=DATE:20261204\r\nDTSTART;VALUE=DATE:20261207\r\nSUMMARY:Aluguel (adiado)\r\nEND:VEVENT\r\n");
        let items = expand(&parse_ics(&raw, OFFSET), days_from_civil(2026, 11, 1), days_from_civil(2027, 1, 31));
        assert!(titles_on(&items, 2026, 12, 4).is_empty());
        assert_eq!(titles_on(&items, 2026, 12, 7), vec!["Aluguel (adiado)"]);
        assert_eq!(titles_on(&items, 2027, 1, 4), vec!["Aluguel"]);
    }

    #[test]
    fn nager_holidays_are_all_day_and_marked() {
        let json = r#"[{"date":"2026-11-15","localName":"Proclamação da República","name":"Republic Proclamation Day"},
                       {"date":"2026-11-20","localName":"Dia da Consciência Negra","name":"x"}]"#;
        let items = parse_holidays(json);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.holiday && i.minute.is_none()));
        assert_eq!(items[0].day, days_from_civil(2026, 11, 15));
        assert_eq!(items[0].title, "Proclamação da República");
    }

    #[test]
    fn civil_days_round_trip_and_weekday() {
        let day = days_from_civil(2026, 11, 11);
        assert_eq!(civil_from_days(day), (2026, 11, 11));
        assert_eq!(weekday(day), 3); // Wednesday
    }
}
