// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Port of the power button's menu from Mac Island & Dock 3.1.1 (PowerAction, PowerActionOn,
// PowerAsks, OnlyPowerAction, PowerPanel::Name/Question/ConfirmName): which actions it offers and
// which ask first. Running them is isle-platform-win's `power::run`.

use serde::{Deserialize, Serialize};

/// The power button's actions, in the menu's order (mac-island's PowerAction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PowerAction {
    Sleep,
    Hibernate,
    Restart,
    ShutDown,
    Lock,
    SignOut,
}

impl PowerAction {
    pub const ALL: [Self; 6] = [Self::Sleep, Self::Hibernate, Self::Restart, Self::ShutDown, Self::Lock, Self::SignOut];

    /// Port of PowerPanel::Name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sleep => "Suspender",
            Self::Hibernate => "Hibernar",
            Self::Restart => "Reiniciar",
            Self::ShutDown => "Desligar",
            Self::Lock => "Bloquear",
            Self::SignOut => "Sair",
        }
    }

    /// Port of PowerPanel::Question (only the actions that ask).
    pub fn question(self) -> &'static str {
        match self {
            Self::Restart => "Reiniciar agora?",
            Self::ShutDown => "Desligar agora?",
            _ => "Sair agora?",
        }
    }
}

/// Each action on or off, and whether restarting, shutting down and signing out ask first
/// (mac-island's islandPower*, all on by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PowerOptions {
    pub sleep: bool,
    pub hibernate: bool,
    pub restart: bool,
    pub shut_down: bool,
    pub lock: bool,
    pub sign_out: bool,
    pub confirm: bool,
}

impl Default for PowerOptions {
    fn default() -> Self {
        Self { sleep: true, hibernate: true, restart: true, shut_down: true, lock: true, sign_out: true, confirm: true }
    }
}

impl PowerOptions {
    /// Port of PowerActionOn.
    pub fn on(&self, action: PowerAction) -> bool {
        match action {
            PowerAction::Sleep => self.sleep,
            PowerAction::Hibernate => self.hibernate,
            PowerAction::Restart => self.restart,
            PowerAction::ShutDown => self.shut_down,
            PowerAction::Lock => self.lock,
            PowerAction::SignOut => self.sign_out,
        }
    }

    /// Port of PowerAsks: restarting, shutting down and signing out ask first, if chosen.
    pub fn asks(&self, action: PowerAction) -> bool {
        self.confirm && matches!(action, PowerAction::Restart | PowerAction::ShutDown | PowerAction::SignOut)
    }

    /// The menu's rows (PowerPanel::Shown): the actions on, Hibernate only when Windows allows it.
    pub fn shown(&self, hibernate_allowed: bool) -> Vec<PowerAction> {
        PowerAction::ALL
            .into_iter()
            .filter(|a| self.on(*a) && (*a != PowerAction::Hibernate || hibernate_allowed))
            .collect()
    }

    /// Port of OnlyPowerAction: the one action shown, if only one is (a click on the button runs it,
    /// unless it asks first).
    pub fn only(&self, hibernate_allowed: bool) -> Option<PowerAction> {
        match self.shown(hibernate_allowed).as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_on_asks_only_for_what_cannot_be_undone() {
        let o = PowerOptions::default();
        assert_eq!(o.shown(true), PowerAction::ALL.to_vec());
        assert_eq!(o.shown(false).len(), 5, "no Hibernate when Windows does not allow it");
        let asking: Vec<_> = PowerAction::ALL.into_iter().filter(|a| o.asks(*a)).collect();
        assert_eq!(asking, [PowerAction::Restart, PowerAction::ShutDown, PowerAction::SignOut]);
        assert!(!PowerOptions { confirm: false, ..o }.asks(PowerAction::ShutDown));
        assert_eq!(o.only(true), None);
    }

    #[test]
    fn a_single_action_left_is_the_buttons_own() {
        let none = PowerOptions { sleep: false, hibernate: false, restart: false, shut_down: false, lock: false, sign_out: false, confirm: true };
        assert_eq!(PowerOptions { lock: true, ..none }.only(true), Some(PowerAction::Lock));
        // Hibernate alone, where Windows does not allow it: nothing to run.
        assert_eq!(PowerOptions { hibernate: true, ..none }.only(false), None);
        assert_eq!(PowerOptions { hibernate: true, lock: true, ..none }.only(false), Some(PowerAction::Lock));
    }
}
