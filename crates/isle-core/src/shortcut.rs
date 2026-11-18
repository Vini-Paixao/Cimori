// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Port of Mac Island & Dock 3.1.1's recorded shortcuts (kShortcutWin/Ctrl/Alt/Shift, ShortcutText,
// KeyName, ControlPanel::OnShortcutRecorded): a key with its modifiers in one number, `key | mods << 8`,
// 0 for none.

use crate::config::Hotkey;

pub const WIN: u32 = 1;
pub const CTRL: u32 = 2;
pub const ALT: u32 = 4;
pub const SHIFT: u32 = 8;

pub const fn code(mods: u32, key: u8) -> u32 {
    key as u32 | mods << 8
}

/// Port of KeyName: letters and digits as themselves, F1–F24, and the usual named keys.
pub fn key_name(vk: u8) -> String {
    match vk {
        b'A'..=b'Z' | b'0'..=b'9' => (vk as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x20 => "Espaço".into(),
        0x0D => "Enter".into(),
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x1B => "Esc".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x25 => "←".into(),
        0x26 => "↑".into(),
        0x27 => "→".into(),
        0x28 => "↓".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        other => format!("Tecla {other:#04x}"),
    }
}

/// Port of ShortcutText (unspaced): "Win+J", "Ctrl+Alt+K"; "Nenhum" for none.
pub fn text(code: u32) -> String {
    if code == 0 {
        return "Nenhum".into();
    }
    let mods = code >> 8;
    let mut out = String::new();
    for (bit, name) in [(WIN, "Win"), (CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift")] {
        if mods & bit != 0 {
            out.push_str(name);
            out.push('+');
        }
    }
    out + &key_name((code & 0xFF) as u8)
}

/// Port of OnShortcutRecorded's rule: without Win, Ctrl or Alt it would take the key from every app.
pub fn accepts(code: u32) -> bool {
    code & 0xFF != 0 && (code >> 8) & (WIN | CTRL | ALT) != 0
}

/// The fixed list the settings had before (and the keyboard hook still handles Win+J and Win+N).
pub fn from_hotkey(h: Hotkey) -> u32 {
    match h {
        Hotkey::None => 0,
        Hotkey::WinJ => code(WIN, b'J'),
        Hotkey::WinN => code(WIN, b'N'),
        Hotkey::CtrlAltJ => code(CTRL | ALT, b'J'),
        Hotkey::CtrlAltN => code(CTRL | ALT, b'N'),
    }
}

/// The old list's entry for keys, if they are one of it; any other combination is `None` there and
/// registered on its own (see the app's shortcuts).
pub fn to_hotkey(code: u32) -> Hotkey {
    Hotkey::ALL.into_iter().find(|h| *h != Hotkey::None && from_hotkey(*h) == code).unwrap_or(Hotkey::None)
}

/// The modifiers RegisterHotKey takes (MOD_ALT 1, MOD_CONTROL 2, MOD_SHIFT 4, MOD_WIN 8) and the key.
pub fn register_args(code: u32) -> (u32, u32) {
    let mods = code >> 8;
    let mut out = 0;
    if mods & ALT != 0 { out |= 1; }
    if mods & CTRL != 0 { out |= 2; }
    if mods & SHIFT != 0 { out |= 4; }
    if mods & WIN != 0 { out |= 8; }
    (out, code & 0xFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_read_as_the_mod_writes_them() {
        assert_eq!(text(0), "Nenhum");
        assert_eq!(text(code(WIN, b'J')), "Win+J");
        assert_eq!(text(code(CTRL | ALT | SHIFT, 0x71)), "Ctrl+Alt+Shift+F2");
        assert_eq!(text(code(WIN | SHIFT, 0xBE)), "Win+Shift+.");
    }

    #[test]
    fn a_key_alone_or_with_shift_is_not_taken() {
        assert!(!accepts(code(0, b'K')));
        assert!(!accepts(code(SHIFT, b'K')));
        assert!(!accepts(code(CTRL, 0)));
        assert!(accepts(code(CTRL, b'K')) && accepts(code(WIN, b'K')) && accepts(code(ALT | SHIFT, b'K')));
    }

    #[test]
    fn the_old_list_maps_both_ways_and_the_rest_is_registered_apart() {
        for h in Hotkey::ALL {
            assert_eq!(to_hotkey(from_hotkey(h)), h);
        }
        assert_eq!(to_hotkey(code(CTRL | SHIFT, b'K')), Hotkey::None);
        assert_eq!(register_args(code(WIN | CTRL, b'K')), (8 | 2, b'K' as u32));
    }
}
