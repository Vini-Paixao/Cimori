// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted from Mac Island & Dock 3.1.1: TrayIcon, TrayIconKey, OpenFromTrayIcon,
// PostTrayIconMessage/Release and pinned/overflow behavior.
// The mod intercepts Shell_NotifyIcon inside explorer.exe. A normal process cannot
// obtain its private owner HWND/callback/uID/version tuple through a supported API.
// We do NOT guess offsets, inject, spoof TaskbarCreated or send guessed callbacks.
// UI Automation resolves fresh shell buttons for Invoke / real right clicks;
// NotifyIconSettings supplies identities, executable paths and PNG snapshots.
// Registry history is filtered by UIA or a running
// executable. The last case is only a candidate: clicks still need a unique live
// button. Shell_NotifyIconGetRect is not an enumeration/liveness API and can
// block on a disconnected shell. See docs/PHASE5-VALIDATION.md for limitations.

use isle_core::config::PinnedTrayApp;
pub use isle_core::tray::TrayItem;
use std::sync::{mpsc, Arc};

#[derive(Debug)]
pub enum TrayCommand {
    Panel(bool),
    Activate(String, bool), // stable key, right button
    Changed,
    /// The pinned apps changed in the settings.
    Pins(Vec<isle_core::config::PinnedTrayApp>),
    Stop,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraySnapshot {
    /// The icons came from the Windhawk bridge.
    pub bridge: bool,
    pub items: Vec<TrayItem>,
    pub error: Option<String>,
}

pub struct TrayService {
    tx: mpsc::Sender<TrayCommand>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl TrayService {
    pub fn send(&self, command: TrayCommand) {
        let _ = self.tx.send(command);
        (self.wake)();
    }
}
impl Drop for TrayService {
    fn drop(&mut self) {
        self.send(TrayCommand::Stop);
    }
}

pub fn spawn(
    pins: Vec<PinnedTrayApp>,
    publish: impl Fn(TraySnapshot) + Send + 'static,
) -> TrayService {
    let (tx, rx) = mpsc::channel();
    #[cfg(windows)]
    {
        match win::Signal::new() {
            Ok(signal) => {
                let signal = Arc::new(signal);
                let wake_signal = signal.clone();
                let changed_tx = tx.clone();
                // The optional Windhawk bridge's icons, when it's installed (see bridge.rs).
                let bridged = bridge::Shared::default();
                let bridge_signal = signal.clone();
                bridge::spawn(bridged.clone(), move || bridge_signal.set());
                std::thread::spawn(move || {
                    if let Err(err) = win::run(pins, rx, changed_tx, signal, bridged, &publish) {
                        tracing::warn!("bandeja: {err:#}");
                        publish(TraySnapshot { bridge: false,
                            error: Some(format!("Bandeja indisponível: {err}")),
                            ..Default::default()
                        });
                    }
                });
                return TrayService {
                    tx,
                    wake: Arc::new(move || wake_signal.set()),
                };
            }
            Err(err) => publish(TraySnapshot { bridge: false,
                error: Some(err.to_string()),
                ..Default::default()
            }),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (pins, rx);
        publish(TraySnapshot { bridge: false,
            error: Some("Bandeja disponível no Windows".into()),
            ..Default::default()
        });
    }
    TrayService {
        tx,
        wake: Arc::new(|| {}),
    }
}

/// Read once without opening overflow, starting services or touching settings/logs.
pub fn inspect(pins: &[PinnedTrayApp]) -> anyhow::Result<TraySnapshot> {
    #[cfg(windows)]
    {
        win::inspect(pins)
    }
    #[cfg(not(windows))]
    {
        let _ = pins;
        anyhow::bail!("Bandeja disponível no Windows")
    }
}

#[cfg(windows)]
mod win;
/// A program's name as Windows shows it (its FileDescription), else its file name.
#[cfg(windows)]
pub use win::program_name;
#[cfg(windows)]
mod bridge;
