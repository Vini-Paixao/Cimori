// SPDX-License-Identifier: GPL-3.0
// The Timer tab's countdown and stopwatch (Mac Island & Dock 3.1.1 has both beside its
// Pomodoro), pure like isle's Pomodoro: the caller passes the time in milliseconds.

use serde::{Deserialize, Serialize};

/// A countdown: set, start, pause, reset; it finishes once (see `tick`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Countdown {
    pub duration_ms: i64,
    /// Set while running.
    ends_at: Option<i64>,
    /// Time left while paused or not started.
    left_ms: i64,
}

impl Countdown {
    pub fn new(duration_ms: i64) -> Self {
        let duration_ms = duration_ms.max(1000);
        Self { duration_ms, ends_at: None, left_ms: duration_ms }
    }
    pub fn running(&self) -> bool { self.ends_at.is_some() }
    /// Started (running or paused partway), as opposed to set and waiting.
    pub fn started(&self) -> bool { self.running() || self.left_ms != self.duration_ms }
    pub fn remaining(&self, now: i64) -> i64 {
        self.ends_at.map_or(self.left_ms, |end| (end - now).max(0))
    }
    pub fn deadline(&self) -> Option<i64> { self.ends_at }
    pub fn start(&mut self, now: i64) {
        if self.ends_at.is_none() && self.left_ms > 0 { self.ends_at = Some(now + self.left_ms); }
    }
    pub fn pause(&mut self, now: i64) {
        if let Some(end) = self.ends_at.take() { self.left_ms = (end - now).max(0); }
    }
    pub fn reset(&mut self) { *self = Self::new(self.duration_ms); }
    /// A new length, only while not started (the − / + and the presets).
    pub fn set_duration(&mut self, duration_ms: i64) {
        if !self.started() { *self = Self::new(duration_ms.clamp(60_000, 24 * 3_600_000)); }
    }
    /// True once, when it reaches zero; it is then set again at its full length.
    pub fn tick(&mut self, now: i64) -> bool {
        match self.ends_at {
            Some(end) if now >= end => { self.reset(); true }
            _ => false,
        }
    }
}

/// A stopwatch: start, pause, reset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stopwatch {
    started_at: Option<i64>,
    banked_ms: i64,
}

impl Stopwatch {
    pub fn running(&self) -> bool { self.started_at.is_some() }
    pub fn elapsed(&self, now: i64) -> i64 {
        self.banked_ms + self.started_at.map_or(0, |at| (now - at).max(0))
    }
    pub fn start(&mut self, now: i64) { self.started_at.get_or_insert(now); }
    pub fn pause(&mut self, now: i64) {
        if let Some(at) = self.started_at.take() { self.banked_ms += (now - at).max(0); }
    }
    pub fn reset(&mut self) { *self = Self::default(); }
}

/// The Timer tab's modes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerMode {
    #[default]
    Pomodoro,
    Countdown,
    Stopwatch,
    Reminders,
}

/// What the Timer tab and the compact bar draw, refreshed by the app on each wake.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimersView {
    pub mode: TimerMode,
    pub pomodoro_phase: crate::pomodoro::Phase,
    pub pomodoro_running: bool,
    pub pomodoro_remaining_ms: i64,
    pub pomodoro_duration_ms: i64,
    pub focus_minutes: u32,
    pub break_minutes: u32,
    pub countdown_running: bool,
    pub countdown_started: bool,
    pub countdown_remaining_ms: i64,
    pub countdown_duration_ms: i64,
    pub stopwatch_running: bool,
    pub stopwatch_ms: i64,
    /// Reminders: the pending ones (soonest first), what's being typed and in how long.
    pub reminders: Vec<ReminderLine>,
    pub reminder_text: String,
    pub reminder_typing: bool,
    pub reminder_minutes: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReminderLine {
    pub id: u64,
    pub text: String,
    /// "às 17:30" (or "amanhã às 08:00").
    pub when: String,
}

/// "4:05", "12:00", "1:02:03": whole seconds, rounded up while counting down.
pub fn clock(ms: i64, round_up: bool) -> String {
    let secs = if round_up { (ms.max(0) + 999) / 1000 } else { ms.max(0) / 1000 };
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_countdown_pauses_resumes_and_finishes_once() {
        let mut c = Countdown::new(5 * 60_000);
        c.start(0);
        assert_eq!(c.remaining(60_000), 4 * 60_000);
        c.pause(60_000);
        assert_eq!(c.remaining(600_000), 4 * 60_000);
        c.set_duration(60_000); // started: the length stays
        assert_eq!(c.duration_ms, 5 * 60_000);
        c.start(600_000);
        assert!(!c.tick(600_000 + 4 * 60_000 - 1));
        assert!(c.tick(600_000 + 4 * 60_000));
        assert!(!c.running() && !c.tick(10_000_000));
        assert_eq!(c.remaining(0), 5 * 60_000);
        c.set_duration(10 * 60_000);
        assert_eq!(c.remaining(0), 10 * 60_000);
    }

    #[test]
    fn a_stopwatch_banks_time_across_pauses() {
        let mut s = Stopwatch::default();
        s.start(1_000);
        s.pause(4_000);
        s.start(10_000);
        assert_eq!(s.elapsed(12_500), 5_500);
        s.reset();
        assert_eq!(s.elapsed(99_000), 0);
    }

    #[test]
    fn clocks_read_like_a_timer() {
        assert_eq!(clock(245_000, false), "4:05");
        assert_eq!(clock(244_001, true), "4:05");
        assert_eq!(clock(3_723_000, false), "1:02:03");
        assert_eq!(clock(-5, true), "0:00");
    }
}
