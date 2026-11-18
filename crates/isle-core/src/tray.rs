// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock 3.1.1: TrayIconKey and strayPinned ordering.
// Rust data shared by the out-of-process Windows adapter and tiny-skia renderer.

use crate::config::PinnedTrayApp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayItem {
    /// Registry record or UIA identity, retained across refreshes (never a row index).
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub tooltip: String,
    pub exe_path: String,
    pub guid: Option<String>,
    pub uid: u32,
    pub png: Vec<u8>,
    pub pinned: bool,
    /// UIA or Shell_NotifyIconGetRect confirmed this icon, rather than only its process.
    pub confirmed: bool,
}

pub fn normalized_path(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase()
}

fn matches_pin(item: &TrayItem, pin: &PinnedTrayApp) -> bool {
    match pin.guid.as_deref().filter(|s| s.starts_with('{')) {
        Some(guid) => item
            .guid
            .as_deref()
            .is_some_and(|g| g.eq_ignore_ascii_case(guid)),
        None => {
            item.guid.is_none()
                && normalized_path(&item.exe_path) == normalized_path(&pin.exe_path)
                && pin.guid.as_deref().unwrap_or("0").parse::<u32>().ok() == Some(item.uid)
        }
    }
}

pub fn order_items(mut items: Vec<TrayItem>, pins: &[PinnedTrayApp]) -> Vec<TrayItem> {
    let mut seen = std::collections::HashSet::new();
    items.sort_by_key(|i| !i.confirmed);
    items.retain(|i| seen.insert(tray_identity(i)));
    items.sort_by_key(|i| {
        (
            pins.iter()
                .position(|p| matches_pin(i, p))
                .unwrap_or(usize::MAX),
            i.display_name.to_lowercase(),
            i.key.clone(),
        )
    });
    label_pins(&mut items, pins);
    items
}

pub fn label_pins(items: &mut [TrayItem], pins: &[PinnedTrayApp]) {
    for item in items {
        if let Some(pin) = pins.iter().find(|p| matches_pin(item, p)) {
            item.pinned = true;
            item.display_name = pin.display_name.clone();
        } else {
            item.pinned = false;
        }
    }
}

pub fn tray_identity(item: &TrayItem) -> String {
    if let Some(guid) = &item.guid {
        format!("guid:{}", guid.to_lowercase())
    } else if !item.exe_path.is_empty() {
        format!("{}|{}", normalized_path(&item.exe_path), item.uid)
    } else {
        item.key.clone()
    }
}

pub fn match_item_button(item: &TrayItem, names: &[String]) -> Option<usize> {
    let hits: std::collections::HashSet<_> = [&item.tooltip, &item.display_name]
        .into_iter()
        .filter_map(|name| unique_tooltip_match(name, names))
        .collect();
    if hits.len() == 1 {
        hits.into_iter().next()
    } else {
        None
    }
}

/// Names are not icon IDs. Reject either several IDs claiming one button, or one
/// historical ID claiming several buttons. Duplicate snapshots of the SAME ID are fine.
pub fn correlate_buttons(items: &[TrayItem], names: &[String]) -> Vec<Option<usize>> {
    use std::collections::{HashMap, HashSet};
    let tentative: Vec<_> = items.iter().map(|i| match_item_button(i, names)).collect();
    let mut by_button: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut by_identity: HashMap<String, HashSet<usize>> = HashMap::new();
    for (item, button) in items.iter().zip(&tentative) {
        if let Some(button) = button {
            by_button
                .entry(*button)
                .or_default()
                .insert(tray_identity(item));
            by_identity
                .entry(tray_identity(item))
                .or_default()
                .insert(*button);
        }
    }
    items
        .iter()
        .zip(tentative)
        .map(|(item, button)| {
            button
                .filter(|b| by_button[b].len() == 1 && by_identity[&tray_identity(item)].len() == 1)
        })
        .collect()
}

/// The shell appends dynamic status to the first tooltip line. Never match arbitrary substrings.
pub fn tooltip_matches(a: &str, b: &str) -> bool {
    let first = |s: &str| {
        s.lines()
            .next()
            .unwrap_or("")
            .split(" — ")
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase()
    };
    !a.trim().is_empty() && !b.trim().is_empty() && first(a) == first(b)
}

pub fn unique_tooltip_match(name: &str, names: &[String]) -> Option<usize> {
    let mut matches = names
        .iter()
        .enumerate()
        .filter(|(_, n)| tooltip_matches(name, n));
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}

pub const INLINE_LIMIT: usize = 6;
pub const TRAY_PER_PAGE: usize = 10;

pub fn inline_indices(items: &[TrayItem]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.pinned)
        .take(INLINE_LIMIT)
        .map(|(n, _)| n)
        .collect()
}

pub fn overflow_indices(items: &[TrayItem]) -> Vec<usize> {
    let inline = inline_indices(items);
    (0..items.len()).filter(|i| !inline.contains(i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PinnedTrayApp;

    fn icon(key: &str, exe: &str, guid: Option<&str>, uid: u32) -> TrayItem {
        TrayItem {
            key: key.into(),
            exe_path: exe.into(),
            guid: guid.map(str::to_owned),
            uid,
            display_name: key.into(),
            ..Default::default()
        }
    }

    #[test]
    fn pins_match_guid_across_exe_updates_and_uid_without_basename_collisions() {
        let pins = vec![
            PinnedTrayApp {
                display_name: "Drive".into(),
                exe_path: r"C:\old\drive.exe".into(),
                guid: Some("{ABCD}".into()),
            },
            PinnedTrayApp {
                display_name: "Deskflow".into(),
                exe_path: r"C:\Apps\deskflow.exe".into(),
                guid: Some("1".into()),
            },
        ];
        let result = order_items(
            vec![
                icon("wrong-exe", r"D:\deskflow.exe", None, 1),
                icon("wrong-uid", r"C:\Apps\deskflow.exe", None, 2),
                icon("desk", r"c:/apps/DESKFLOW.exe", None, 1),
                icon("drive", r"C:\new\drive.exe", Some("{abcd}"), 0),
            ],
            &pins,
        );
        assert_eq!(
            result.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
            ["drive", "desk", "wrong-exe", "wrong-uid"]
        );
        assert!(result[0].pinned && result[1].pinned);
        assert!(!result[2].pinned && !result[3].pinned);
    }

    #[test]
    fn absent_pins_are_not_launchers_and_duplicate_snapshots_do_not_duplicate_icons() {
        let pins = vec![PinnedTrayApp {
            display_name: "Gone".into(),
            exe_path: "gone.exe".into(),
            guid: None,
        }];
        let i = icon("same", "live.exe", None, 0);
        let result = order_items(vec![i.clone(), i], &pins);
        assert_eq!(result.len(), 1);
        assert!(!result[0].pinned);
        assert!(order_items(Vec::new(), &pins).is_empty());
    }

    #[test]
    fn live_button_matching_rejects_substrings_and_ambiguous_names() {
        assert!(tooltip_matches("Steam\nOnline", "Steam"));
        assert!(!tooltip_matches("SteamVR", "Steam"));
        assert!(!tooltip_matches("", ""));
        assert_eq!(
            unique_tooltip_match("Steam", &["Steam\nOnline".into(), "Discord".into()]),
            Some(0)
        );
        assert_eq!(
            unique_tooltip_match("Steam", &["Steam\nOnline".into(), "Steam\nOffline".into()]),
            None
        );
    }

    #[test]
    fn duplicated_registry_history_uses_the_live_executable_record() {
        let mut old = icon("old", r"C:\v1\drive.exe", Some("{abcd}"), 0);
        old.confirmed = false;
        let mut live = icon("live", r"C:\v2\drive.exe", Some("{ABCD}"), 0);
        live.confirmed = true;
        let result = order_items(vec![old, live], &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "live");
    }

    #[test]
    fn extra_pins_stay_reachable_in_overflow() {
        let items: Vec<_> = (0..9)
            .map(|i| TrayItem {
                key: i.to_string(),
                pinned: true,
                ..Default::default()
            })
            .collect();
        assert_eq!(inline_indices(&items), [0, 1, 2, 3, 4, 5]);
        assert_eq!(overflow_indices(&items), [6, 7, 8]);
    }

    #[test]
    fn one_button_cannot_confirm_two_different_registry_icon_ids() {
        let mut a = icon("a", "app.exe", None, 1);
        a.tooltip = "App".into();
        let mut b = a.clone();
        b.key = "b".into();
        b.uid = 2;
        assert_eq!(
            correlate_buttons(&[a.clone(), b], &["App".into()]),
            [None, None]
        );
        let mut same = a.clone();
        same.key = "old-record".into();
        assert_eq!(
            correlate_buttons(&[a, same], &["App".into()]),
            [Some(0), Some(0)]
        );
    }

    #[test]
    fn one_registry_identity_cannot_claim_two_different_live_buttons() {
        let mut a = icon("a", "app.exe", Some("{guid}"), 0);
        a.tooltip = "App one".into();
        let mut b = a.clone();
        b.key = "b".into();
        b.tooltip = "App two".into();
        assert_eq!(
            correlate_buttons(&[a, b], &["App one".into(), "App two".into()]),
            [None, None]
        );
    }
}
