//! Platform-independent core of Cimori (isle-win).
//!
//! Upstream modules are ported unchanged from zickrian/isle @ a29a7c4 (v1.7.1, GPL-3.0-or-later);
//! keep them byte-identical so later upstream fixes can be copied over. isle-win's own modules
//! (Windhawk config import, Google Calendar ICS, pill spring and state) are listed separately.

// Upstream (zickrian/isle).
pub mod app_usage;
pub mod battery;
pub mod calendar;
pub mod clipboard;
pub mod lrc;
pub mod pomodoro;
pub mod reminders;
pub mod shelf;
pub mod speech;
pub mod todo;
pub mod track;
pub mod voice;

// isle-win.
pub mod config;
pub mod agenda;
pub mod spring;
pub mod state;
pub mod notifications;
pub mod tray;
pub mod weather;
pub mod timers;
pub mod chat;
pub mod data_dir;

pub mod artwork;
pub mod clock;
pub mod prefs;

pub mod phase8;
pub mod power;
pub mod shortcut;

pub mod icons;
pub mod updates;
