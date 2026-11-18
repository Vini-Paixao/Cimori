// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock v3.1.0: ToastTexts, SplitToastTexts and ToastThreadProc.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalShortcut {
    WinI, // AI Hub
    WinN, // Notifications & Calendar
    WinA, // Control Center
    AltM, // Minimize pill (code 330 from Windhawk backup)
}

/// The mod's bounded <text> reader, not a general XML parser. Never resolves external entities.
pub fn parse_toast(payload: &[u8]) -> Option<(String, String)> {
    if payload.len() > 1_048_576 { return None; }
    let mut xml = std::str::from_utf8(payload).ok()?;
    let mut texts = Vec::new();
    while let Some(start) = find_markup(xml, "<text") {
        xml = &xml[start + 5..];
        if !xml.starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/') { continue; }
        let mut quote = None;
        let end = xml.char_indices().find_map(|(i, c)| {
            if Some(c) == quote { quote = None; }
            else if quote.is_none() {
                if c == '\'' || c == '"' { quote = Some(c); }
                else if c == '>' { return Some(i); }
            }
            None
        })?;
        let attributes = &xml[..end];
        xml = &xml[end + 1..];
        if attributes.trim_end().ends_with('/') { continue; }
        let close = find_markup(xml, "</text>")?;
        let raw = &xml[..close];
        xml = &xml[close + 7..];
        if attribution(attributes)? { continue; }
        let text = decode_xml(raw)?.replace('\r', "").replace('\t', " ");
        let text = text.trim().to_owned();
        if !text.is_empty() { texts.push(text); }
    }
    if texts.is_empty() { return None; }
    // Bound retained content before sending it to the renderer (a toast payload may contain huge text).
    let title = texts.remove(0).replace('\n', " ").chars().take(256).collect();
    Some((title, texts.join("\n").chars().take(1024).collect()))
}

/// Markup-looking text in comments/CDATA is not an element or a closing tag.
fn find_markup(mut xml: &str, marker: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(next) = xml.find('<') {
        offset += next;
        xml = &xml[next..];
        let skip = if xml.starts_with("<!--") { xml.find("-->")? + 3 }
            else if xml.starts_with("<![CDATA[") { xml.find("]]>")? + 3 }
            else {
                if xml.starts_with(marker) { return Some(offset); }
                1
            };
        offset += skip;
        xml = &xml[skip..];
    }
    None
}

fn attribution(attributes: &str) -> Option<bool> {
    Some(attribute(attributes, "placement")? == Some("attribution"))
}

/// One attribute's raw value; outer None for malformed attributes.
fn attribute<'a>(mut attributes: &'a str, wanted: &str) -> Option<Option<&'a str>> {
    while !attributes.trim().is_empty() {
        attributes = attributes.trim_start();
        if attributes == "/" { break; }
        let name_end = attributes.find(|c: char| c == '=' || c.is_ascii_whitespace())?;
        let name = &attributes[..name_end];
        attributes = attributes[name_end..].trim_start().strip_prefix('=')?.trim_start();
        let quote = attributes.chars().next()?;
        if quote != '\'' && quote != '"' { return None; }
        attributes = &attributes[1..];
        let end = attributes.find(quote)?;
        if name == wanted { return Some(Some(&attributes[..end])); }
        attributes = &attributes[end + 1..];
    }
    Some(None)
}

/// The mod's reply.launch/launchType: `<toast launch="…" activationType="…">`, entities decoded.
pub fn toast_launch(payload: &[u8]) -> (String, String) {
    let read = || -> Option<(String, String)> {
        let xml = std::str::from_utf8(payload).ok()?;
        let tag = &xml[find_markup(xml, "<toast")? + 6..];
        let attributes = &tag[..tag.find('>')?];
        let get = |name| attribute(attributes, name).flatten().and_then(decode_xml).unwrap_or_default();
        Some((get("launch").chars().take(4096).collect(), get("activationType")))
    };
    read().unwrap_or_default()
}

/// The mod's ParseToastReply, reply part: the first text `<input>` and the `<action>` whose
/// hint-inputId names it (its arguments go back to the app with the text). Empty without both,
/// and for a link button ("protocol"), which can't carry text.
pub fn toast_reply(payload: &[u8]) -> (String, String) {
    let read = || -> Option<(String, String)> {
        let xml = std::str::from_utf8(payload).ok()?;
        let tags = |name: &'static str| {
            let mut rest = xml;
            std::iter::from_fn(move || loop {
                let at = find_markup(rest, name)? + name.len();
                let tag = &rest[at..];
                let end = tag.find('>')?;
                rest = &tag[end..];
                // `<actions>` is not an `<action`.
                if tag.starts_with(|c: char| c.is_ascii_whitespace()) { return Some(&tag[..end]); }
            })
        };
        let get = |attributes, name| attribute(attributes, name).flatten().and_then(decode_xml);
        let input = tags("<input").find(|a| get(a, "type").as_deref() == Some("text")).and_then(|a| get(a, "id"))?;
        let action = tags("<action").find(|a| get(a, "hint-inputId").as_deref() == Some(input.as_str())
            && get(a, "activationType").as_deref() != Some("protocol"))?;
        Some((input, get(action, "arguments")?.chars().take(4096).collect()))
    };
    read().unwrap_or_default()
}

/// The mod's SendReply: the app's activator gets the send button's arguments and the text.
pub fn send_reply(app_id: String, arguments: String, input_id: String, text: String) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        match native::activate(&app_id, &arguments, Some((&input_id, &text))) {
            Ok(()) => tracing::info!("resposta para {app_id}: entregue ao ativador do app"),
            Err(err) => tracing::warn!("resposta para {app_id}: {err:#}"),
        }
        let _ = (&app_id, &arguments, &input_id, &text);
    });
}

/// `ToastActivatorCLSID="…"` in a packaged app's AppxManifest.xml: its notification activator.
pub fn toast_activator(manifest: &str) -> Option<&str> {
    let start = manifest.find("ToastActivatorCLSID=\"")? + 21;
    let clsid = &manifest[start..start + manifest[start..].find('"')?];
    (clsid.len() == 36).then_some(clsid)
}

fn decode_xml(mut raw: &str) -> Option<String> {
    let mut text = String::new();
    while !raw.is_empty() {
        if let Some(comment) = raw.strip_prefix("<!--") {
            raw = &comment[comment.find("-->")? + 3..];
        } else if let Some(cdata) = raw.strip_prefix("<![CDATA[") {
            let end = cdata.find("]]>")?;
            text.push_str(&cdata[..end]);
            raw = &cdata[end + 3..];
        } else if let Some(entity) = raw.strip_prefix('&') {
            let end = entity.find(';')?;
            let value = &entity[..end];
            text.push(match value {
                "amp" => '&', "lt" => '<', "gt" => '>', "quot" => '"', "apos" => '\'',
                _ => {
                    let code = if let Some(hex) = value.strip_prefix("#x") {
                        u32::from_str_radix(hex,16).ok()?
                    } else { value.strip_prefix('#')?.parse().ok()? };
                    char::from_u32(code).filter(|c| *c != '\0')?
                }
            });
            raw = &entity[end + 1..];
        } else {
            let c = raw.chars().next()?;
            if c == '<' || c == '\0' { return None; }
            text.push(c);
            raw = &raw[c.len_utf8()..];
        }
    }
    Some(text)
}

/// The mod's OpenNotification, last step: the app by its ID through the shell (opens it, or brings
/// its window forward). None for a sender known only by a path or a CLSID, as there.
pub fn app_target(app_id: &str) -> Option<String> {
    (!app_id.is_empty() && !app_id.contains('\\') && !app_id.starts_with('{'))
        .then(|| format!("shell:AppsFolder\\{app_id}"))
}

pub fn open_app(app_id: &str) -> anyhow::Result<()> {
    let target = app_target(app_id).ok_or_else(|| anyhow::anyhow!("app sem atalho no Windows"))?;
    std::process::Command::new("explorer.exe").arg(target).spawn()?;
    Ok(())
}

/// Port of ActivateToast's protocol check (mac-island 3.1.1): only a URI with a scheme of two or more
/// letters, digits, "+", "-" or "." ("https:", an app's own), never a file or a path.
pub fn protocol_uri_ok(launch: &str) -> bool {
    // Hardening beyond the mod: schemes known to run code or search the disk from a crafted link.
    const DENIED: &[&str] = &["file", "search", "search-ms", "ms-msdt", "ms-officecmd", "ms-appinstaller", "shell",
        "ms-word", "ms-excel", "ms-powerpoint", "ms-access", "ms-visio", "ms-project", "ms-publisher", "ms-infopath",
        "ms-cxh", "ms-cxh-full", "vbscript", "javascript"];
    match launch.find(':') {
        Some(colon) if colon >= 2 => {
            let scheme = &launch[..colon];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                && !DENIED.iter().any(|d| d.eq_ignore_ascii_case(scheme))
        }
        _ => false,
    }
}

/// The mod's OpenNotification: the app's own notification activator gets the toast's `launch`
/// (WhatsApp opens on the chat) -- a packaged app's from its manifest, an unpackaged one's from its
/// registration or Start menu shortcut (ToastActivatorCLSID); `activationType="protocol"` opens the
/// URI; the app itself when none of that can be done. Off the UI thread.
pub fn open_notification(app_id: String, launch: String, launch_type: String) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        if launch_type == "protocol" {
            if protocol_uri_ok(&launch) {
                match native::open_uri(&launch) {
                    Ok(()) => return,
                    Err(err) => tracing::warn!("abrir o link do aviso de {app_id}: {err:#}"),
                }
            }
        } else if !launch.is_empty() && matches!(launch_type.as_str(), "" | "foreground" | "background") {
            match native::activate(&app_id, &launch, None) {
                Ok(()) => return,
                Err(err) => tracing::warn!("ativar aviso de {app_id}: {err:#}"),
            }
        }
        let _ = (&launch, &launch_type);
        if let Err(err) = open_app(&app_id) { tracing::warn!("abrir {app_id}: {err:#}"); }
    });
}

/// `<badge value="2"/>`: the app's taskbar icon shows something (a count or a glyph), not "0"/"none".
pub fn badge_shown(payload: &[u8]) -> bool {
    let Ok(xml) = std::str::from_utf8(payload) else { return false; };
    let Some(start) = xml.find("value=\"") else { return false; };
    let value = xml[start + 7..].split('"').next().unwrap_or("");
    !matches!(value, "" | "0" | "none")
}

pub fn arrival_ms(filetime: i64) -> i64 {
    (filetime / 10_000).saturating_sub(11_644_473_600_000).max(0)
}

#[derive(Debug)]
pub enum NotificationEvent {
    AppIcon { app_id:String, image:std::sync::Arc<isle_core::artwork::Artwork> },
    /// `present`: every toast id still in Windows' database; `badged`: the apps whose icon shows a badge.
    Batch { items: Vec<isle_core::notifications::Notification>, initial: bool, present: Vec<i64>, badged: Vec<String> },
    Status(Option<String>),
    Open,
    Minimize,
}

#[cfg(windows)]
#[path = "notification_windows.rs"]
mod native;

/// `win_n`: take Win+N for the island; `icons`: look up each sender's icon.
pub fn spawn(win_n: bool, icons: bool, send: impl Fn(NotificationEvent) + Send + Sync + 'static) {
    #[cfg(windows)]
    native::spawn(std::sync::Arc::new(send), win_n, icons);
    #[cfg(not(windows))]
    let _ = (win_n, icons);
    #[cfg(not(windows))]
    send(NotificationEvent::Status(Some("Notificações disponíveis no Windows".into())));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    OpenNotifications,
    ToggleMinimize,
}

/// Assigns Cimori actions to Win+N and Win+J. An unassigned key follows the
/// normal Windows shortcut path. Settings apply to the next fresh key press.
pub fn configure_shortcuts(
    win_n: Option<ShortcutAction>,
    win_j: Option<ShortcutAction>,
    publish: impl Fn(NotificationEvent) + Send + Sync + 'static,
) {
    #[cfg(windows)]
    native::configure_shortcuts(win_n, win_j, std::sync::Arc::new(publish));
    #[cfg(not(windows))]
    let _ = (win_n, win_j, publish);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(windows, test))]
enum KeyDisposition {
    Pass,
    Swallow,
    Trigger(ShortcutAction),
}

#[derive(Default, Clone, Copy)]
#[cfg(any(windows, test))]
enum KeyPress {
    #[default]
    Released,
    Passed,
    Captured,
}

#[derive(Default)]
#[cfg(any(windows, test))]
struct ShortcutState {
    n: KeyPress,
    j: KeyPress,
}

#[cfg(any(windows, test))]
impl ShortcutState {
    fn press(&mut self, vk: u32) -> Option<&mut KeyPress> {
        match vk {
            0x4e => Some(&mut self.n),
            0x4a => Some(&mut self.j),
            _ => None,
        }
    }

    fn route(
        &mut self,
        vk: u32,
        down: bool,
        injected: bool,
        win_only: bool,
        action: Option<ShortcutAction>,
    ) -> KeyDisposition {
        if injected {
            return KeyDisposition::Pass;
        }
        let Some(press) = self.press(vk) else {
            return KeyDisposition::Pass;
        };
        if !down {
            return match std::mem::take(press) {
                KeyPress::Captured => KeyDisposition::Swallow,
                _ => KeyDisposition::Pass,
            };
        }
        match press {
            KeyPress::Captured => KeyDisposition::Swallow,
            KeyPress::Passed => KeyDisposition::Pass,
            KeyPress::Released => {
                if let Some(action) = action.filter(|_| win_only) {
                    *press = KeyPress::Captured;
                    KeyDisposition::Trigger(action)
                } else {
                    *press = KeyPress::Passed;
                    KeyDisposition::Pass
                }
            }
        }
    }

    fn pass_press(&mut self, vk: u32) {
        if let Some(press) = self.press(vk) {
            *press = KeyPress::Passed;
        }
    }
}

#[derive(Default)]
#[cfg(any(windows, test))]
struct ReadCursor { last: Option<i64> }

#[cfg(any(windows, test))]
impl ReadCursor {
    fn query_after(&self, max_id: i64) -> (i64, bool) {
        match self.last { Some(last) if max_id >= last => (last,false), _ => (0,true) }
    }
    fn commit(&mut self, max_id: i64) { self.last = Some(max_id); }
}

#[cfg(any(windows, test))]
fn fallback_app_name(app: &str) -> String {
    let file = app.rsplit(['\\','/']).next().unwrap_or(app);
    if file.to_ascii_lowercase().ends_with(".exe") { return file[..file.len()-4].to_owned(); }
    let package = file.split('!').next().unwrap_or(file).split('_').next().unwrap_or(file);
    package.rsplit('.').next().unwrap_or(package).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_protocol_launch_is_a_uri_never_a_file_or_a_path() {
        for ok in ["https://example.com/x", "whatsapp:chat?id=1", "ms-settings:network", "my-app.v2+x://a"] {
            assert!(protocol_uri_ok(ok), "{ok}");
        }
        for bad in ["", "C:\\Windows\\notepad.exe", "C:/x", "file:///C:/x", "FILE:x", "a:b", "no scheme", "bad scheme:x", "/usr/bin", "\\\\server\\share",
            "search-ms:query=x", "ms-msdt:/id", "MS-OfficeCmd:x", "ms-word:ofe|u|http://x", "1ab:x", "çã:x"] {
            assert!(!protocol_uri_ok(bad), "{bad}");
        }
    }

    #[test]
    fn launch_arguments_and_activator_come_from_the_toast_and_manifest() {
        let toast = br#"<?xml version="1.0"?><toast launch="action=Click&amp;key=msg%3Afalse_1%40lid&amp;tag=1%40lid"><visual><text>Amor</text></visual></toast>"#;
        assert_eq!(toast_launch(toast), ("action=Click&key=msg%3Afalse_1%40lid&tag=1%40lid".into(), String::new()));
        assert_eq!(toast_launch(br#"<toast activationType='protocol' launch='https://example.com/'/>"#),
            ("https://example.com/".into(), "protocol".into()));
        assert_eq!(toast_launch(b"<toast><visual/></toast>"), Default::default());
        let manifest = r#"<com:Class Id="x"/><desktop:ToastNotificationActivation ToastActivatorCLSID="bebd6a5e-1b14-4a57-a195-c772d28b6d15" />"#;
        assert_eq!(toast_activator(manifest), Some("bebd6a5e-1b14-4a57-a195-c772d28b6d15"));
        assert_eq!(toast_activator("<Package/>"), None);
    }

    #[test]
    fn whatsapp_reply_box_and_send_button_are_found() {
        // WhatsApp's payload, texts and arguments made up.
        let toast = br#"<toast launch="a"><visual><binding><text>Amor</text></binding></visual><actions><input id="quickReplyInputBoxId" type="text" placeHolderContent="Digite uma resposta" /><action content="Enviar" arguments="action=Reply&amp;key=1%40lid" activationType="background" hint-inputId="quickReplyInputBoxId" /><action content="Silenciar" arguments="mute" activationType="background" placement="contextMenu" /></actions></toast>"#;
        assert_eq!(toast_reply(toast), ("quickReplyInputBoxId".into(), "action=Reply&key=1%40lid".into()));
        assert_eq!(toast_reply(b"<toast><visual><text>Oi</text></visual></toast>"), Default::default());
        let link = br#"<toast><actions><input id="r" type="text"/><action arguments="https://x" activationType="protocol" hint-inputId="r"/></actions></toast>"#;
        assert_eq!(toast_reply(link), Default::default());
    }

    #[test]
    fn apps_open_through_the_shell_but_paths_and_clsids_do_not() {
        assert_eq!(app_target("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App").as_deref(),
            Some(r"shell:AppsFolder\5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"));
        assert_eq!(app_target(r"C:\scripts\notify.exe"), None);
        assert_eq!(app_target("{6D809377-6AF0-444B-8957-A3773F02200E}"), None);
        assert_eq!(app_target(""), None);
    }

    #[test]
    fn badge_counts_and_glyphs_show_but_zero_and_none_do_not() {
        assert!(badge_shown(br#"<badge value="2"/>"#));
        assert!(badge_shown(br#"<badge value="newMessage"/>"#));
        assert!(!badge_shown(br#"<badge value="0"/>"#));
        assert!(!badge_shown(br#"<badge value="none"/>"#));
        assert!(!badge_shown(b"<badge/>"));
    }

    #[test]
    fn toast_parser_keeps_literal_closing_tags_in_cdata_and_ignores_comments() {
        let xml = b"<toast><!-- <text>debug</text> --><text><![CDATA[Use </text> literally]]></text><text>A<!-- ignored -->B</text></toast>";
        assert_eq!(parse_toast(xml),Some(("Use </text> literally".into(),"AB".into())));
    }

    #[test]
    fn oversized_text_is_bounded_before_it_reaches_the_ui() {
        let xml = format!("<toast><text>{}</text><text>{}</text></toast>","é".repeat(300),"🙂".repeat(2000));
        let (title,body) = parse_toast(xml.as_bytes()).unwrap();
        assert_eq!(title.chars().count(),256);
        assert_eq!(body.chars().count(),1024);
        assert_eq!(parse_toast(&vec![b'x';1_048_577]),None);
    }

    #[test]
    fn unknown_senders_use_readable_aumid_or_executable_name() {
        assert_eq!(fallback_app_name("Microsoft.WindowsTerminal_8weky!App"),"WindowsTerminal");
        assert_eq!(fallback_app_name("C:\\Apps\\Chat.exe"),"Chat");
        assert_eq!(fallback_app_name("org.example.Mail"),"Mail");
    }

    #[test]
    fn failed_reads_keep_the_cursor_and_database_reset_seeds_silently() {
        let mut cursor = ReadCursor::default();
        assert_eq!(cursor.query_after(20),(0,true));
        cursor.commit(20);
        assert_eq!(cursor.query_after(23),(20,false));
        // A failed read never commits: the next try must include those same IDs.
        assert_eq!(cursor.query_after(25),(20,false));
        assert_eq!(cursor.query_after(3),(0,true));
        cursor.commit(3);
        assert_eq!(cursor.query_after(4),(3,false));
    }

    #[test]
    fn win_n_takes_one_press_and_its_release_even_after_win_is_released() {
        use KeyDisposition::*;
        let mut state = ShortcutState::default();
        let action = Some(ShortcutAction::OpenNotifications);
        assert_eq!(state.route(0x4e, true, false, true, action), Trigger(action.unwrap()));
        assert_eq!(state.route(0x4e, true, false, true, action), Swallow);
        assert_eq!(state.route(0x4e, false, false, false, action), Swallow);
        assert_eq!(state.route(0x4e, true, false, false, action), Pass);
    }

    #[test]
    fn win_n_passes_injected_keys_plain_n_and_extra_modifiers() {
        let mut state = ShortcutState::default();
        let action = Some(ShortcutAction::OpenNotifications);
        assert_eq!(state.route(0x4e, true, true, true, action), KeyDisposition::Pass);
        assert_eq!(state.route(0x4e, true, false, false, action), KeyDisposition::Pass);
        assert_eq!(state.route(0x41, true, false, true, action), KeyDisposition::Pass);
        assert_eq!(state.route(0x4e, false, false, true, action), KeyDisposition::Pass);
    }

    #[test]
    fn remaps_windows_shortcuts_live_without_replaying_a_held_key() {
        use KeyDisposition::*;
        use ShortcutAction::*;
        let mut state = ShortcutState::default();
        assert_eq!(state.route(b'N'.into(), true, false, true, Some(OpenNotifications)), Trigger(OpenNotifications));
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(ToggleMinimize)), Trigger(ToggleMinimize));
        assert_eq!(state.route(b'N'.into(), true, false, true, None), Swallow);
        assert_eq!(state.route(b'N'.into(), false, false, false, None), Swallow);
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(OpenNotifications)), Swallow);
        assert_eq!(state.route(b'J'.into(), false, false, false, Some(OpenNotifications)), Swallow);
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(OpenNotifications)), Trigger(OpenNotifications));
    }

    #[test]
    fn a_passed_or_failed_press_keeps_passing_until_released() {
        use KeyDisposition::*;
        use ShortcutAction::*;
        let mut state = ShortcutState::default();
        assert_eq!(state.route(b'J'.into(), true, false, false, Some(ToggleMinimize)), Pass);
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(ToggleMinimize)), Pass);
        assert_eq!(state.route(b'J'.into(), false, false, true, Some(ToggleMinimize)), Pass);
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(ToggleMinimize)), Trigger(ToggleMinimize));
        state.pass_press(b'J'.into());
        assert_eq!(state.route(b'J'.into(), true, false, true, Some(ToggleMinimize)), Pass);
        assert_eq!(state.route(b'J'.into(), false, false, false, Some(ToggleMinimize)), Pass);
    }

    #[test]
    fn toast_parser_decodes_entities_and_keeps_all_message_lines() {
        let xml = br#"<toast><visual><binding><text hint-style="title">Build &amp; test</text><text>Ol&#225; &#x1F44B;</text><text><![CDATA[A < B]]></text><text placement = 'attribution'>Mail</text></binding></visual></toast>"#;
        assert_eq!(parse_toast(xml), Some(("Build & test".into(), "Olá 👋\nA < B".into())));
    }

    #[test]
    fn toast_parser_ignores_empty_and_lookalike_elements_and_normalizes_whitespace() {
        assert_eq!(parse_toast(b"<toast><texture>wrong</texture><text/><text>Title\r\nnext</text><text>Body\tend</text></toast>"),
            Some(("Title next".into(),"Body end".into())));
        assert_eq!(parse_toast(b"<toast><text placement=\"attribution\">App</text></toast>"), None);
    }

    #[test]
    fn toast_parser_rejects_corrupt_payloads_without_panicking() {
        for bytes in [b"<text>missing close".as_slice(), b"<text>&unknown;</text>", b"\xff", b"<text>&#xD800;</text>"] {
            assert_eq!(parse_toast(bytes),None);
        }
    }

    #[test]
    fn database_filetime_converts_to_unix_milliseconds() {
        assert_eq!(arrival_ms(116_444_736_000_000_000), 0);
        assert_eq!(arrival_ms(116_444_736_012_340_000), 1234);
        assert_eq!(arrival_ms(0), 0);
    }
}

/// What's on the clipboard as text (Ctrl+V in a reply).
pub fn clipboard_text() -> String {
    #[cfg(windows)]
    return native::clipboard_text();
    #[cfg(not(windows))]
    String::new()
}
