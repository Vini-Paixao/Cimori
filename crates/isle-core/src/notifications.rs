// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock v3.1.0: kept toasts, AddUnread and NotificationPanel grouping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    #[serde(skip)]
    pub app_icon: Option<std::sync::Arc<crate::artwork::Artwork>>,
    pub id: i64,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub arrival_ms: i64,
    pub unread: bool,
    /// The `<toast launch="…" activationType="…">` the app gets back when it's clicked (WhatsApp: the chat).
    #[serde(default)]
    pub launch: String,
    #[serde(default)]
    pub launch_type: String,
    /// The mod's ToastReply: the text box's id and the arguments of the button that sends it
    /// (WhatsApp's quickReplyInputBoxId and "Enviar"); empty when it can't be answered.
    #[serde(default)]
    pub reply_input: String,
    #[serde(default)]
    pub reply_args: String,
}

impl Notification {
    pub fn can_reply(&self) -> bool { !self.reply_input.is_empty() && !self.reply_args.is_empty() }
}

/// A reply being typed under a notification (the mod's m_replyId and m_replyText).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub id: i64,
    pub arrival_ms: i64,
    pub text: String,
}

impl Reply {
    /// The mod's EditTextChar, without a caret: what one typed character does. Enter is the
    /// caller's (it sends). At most 2000 characters, like there.
    pub fn type_char(&mut self, c: char) {
        match c {
            '\u{8}' => { self.text.pop(); }
            c if c == '\t' || !c.is_control() => if self.text.chars().count() < 2000 { self.text.push(c); },
            _ => {}
        }
    }
    pub fn paste(&mut self, text: &str) {
        for c in text.replace("\r\n", " ").replace(['\r', '\n'], " ").chars() { self.type_char(c); }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationCenter {
    items: Vec<Notification>,
    seen: Vec<(i64, i64)>,
    /// The mod's m_clearedUntil: "Limpar tudo" hides everything that arrived up to here, across restarts.
    pub cleared_until: i64,
    /// Rows per page in the Avisos tab (settings); 0 reads as PER_PAGE.
    #[serde(skip)]
    pub per_page: usize,
}

impl NotificationCenter {
    /// Artwork arriving from the Shell must not create another toast or mark one unread again.
    pub fn set_app_icon(&mut self,app_id:&str,image:std::sync::Arc<crate::artwork::Artwork>){
        for n in &mut self.items{if n.app_id.eq_ignore_ascii_case(app_id){n.app_icon=Some(image.clone());}}
    }
    /// Initial database history is silent and read; only arrivals during this run are unread.
    pub fn receive(&mut self, items: Vec<Notification>, initial: bool) -> Option<Notification> {
        let mut newest: Option<Notification> = None;
        for mut item in items {
            let key = (item.id, item.arrival_ms);
            if self.seen.contains(&key) || item.arrival_ms <= self.cleared_until { continue; }
            self.seen.push(key);
            item.unread = !initial;
            if !initial && newest.as_ref().is_none_or(|n| (item.arrival_ms, item.id) > (n.arrival_ms, n.id)) {
                newest = Some(item.clone());
            }
            self.items.push(item);
        }
        self.items.sort_by_key(|n| std::cmp::Reverse((n.arrival_ms, n.id)));
        self.items.truncate(100);
        // shortcut: the worker advances its DB cursor; keep 512 recent keys for duplicate deliveries.
        if self.seen.len() > 512 { self.seen.drain(..self.seen.len() - 512); }
        newest
    }

    /// Apps ordered by their latest arrival; within each app, newest first.
    pub fn ordered(&self) -> Vec<&Notification> {
        let mut ordered = Vec::with_capacity(self.items.len());
        for item in &self.items {
            if ordered.iter().any(|n: &&Notification| n.app_id.eq_ignore_ascii_case(&item.app_id)) { continue; }
            ordered.extend(self.items.iter().filter(|n| n.app_id.eq_ignore_ascii_case(&item.app_id)));
        }
        ordered
    }
    pub fn unread_count(&self) -> usize { self.items.iter().filter(|n| n.unread).count() }
    pub fn app_unread(&self, app: &str) -> usize {
        self.items.iter().filter(|n| n.unread && n.app_id.eq_ignore_ascii_case(app)).count()
    }
    pub fn mark_read(&mut self, id: i64, arrival_ms: i64) {
        if let Some(n) = self.items.iter_mut().find(|n| n.id == id && n.arrival_ms == arrival_ms) { n.unread = false; }
    }
    /// Windows' database is the truth, like the mod's panel: a toast the app or the user
    /// removed there leaves the island too, except while its app's icon still shows a badge
    /// (the mod's MergeKeptToasts): WhatsApp replaces a chat's toast with each new message.
    // shortcut: no cap at the badge's number (WhatsApp counts chats, not messages) and no 10 s
    // grace for a badge that lands after its toast is gone; add either if an app needs it.
    pub fn retain_present(&mut self, present: &[i64], badged: &[String]) {
        self.items.retain(|n| present.contains(&n.id) || badged.iter().any(|app| app.eq_ignore_ascii_case(&n.app_id)));
    }
    /// Clearing affects this island's in-memory history, never Windows' database.
    pub fn clear(&mut self) {
        let newest = self.items.iter().map(|n| n.arrival_ms).max().unwrap_or(0);
        self.cleared_until = self.cleared_until.max(newest);
        self.items.clear();
    }
    /// The Avisos tab's last page (0-based) for this many notifications.
    pub fn last_page(&self) -> usize { self.items.len().saturating_sub(1) / self.per_page() }
    pub fn per_page(&self) -> usize { if self.per_page == 0 { PER_PAGE } else { self.per_page } }
}

/// Notifications per page in the Avisos tab.
pub const PER_PAGE: usize = 4;

pub fn notification_age(arrival_ms: i64, now_ms: i64) -> String {
    let minutes = now_ms.saturating_sub(arrival_ms).max(0) / 60_000;
    match minutes {
        0 => crate::i18n::tr("time.now").into(),
        1..=59 => format!("{minutes} min"),
        60..=1439 => format!("{} h", minutes / 60),
        _ => format!("{} d", minutes / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(id: i64, app: &str) -> Notification {
        Notification { id, app_id: app.into(), app_name: app.into(), title: format!("Title {id}"),
            body: "Body".into(), arrival_ms: id * 1000, unread: false, ..Default::default() }
    }

    #[test]
    fn items_windows_removed_leave_the_island_and_do_not_come_back() {
        let mut center = NotificationCenter::default();
        center.receive(vec![notice(1,"WhatsApp"),notice(2,"Defender")], false);
        // WhatsApp cleared its toast after the message was read in the app.
        center.retain_present(&[2], &[]);
        assert_eq!(center.ordered().iter().map(|n| n.id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(center.unread_count(), 1);
        assert!(center.receive(vec![notice(1,"WhatsApp")], false).is_none());
    }

    #[test]
    fn replaced_toasts_stay_while_the_app_shows_a_badge() {
        let mut center = NotificationCenter::default();
        // Three messages in one chat: each toast replaces the last one in Windows' database.
        for id in 1..=3 {
            center.receive(vec![notice(id,"WhatsApp")], false);
            center.retain_present(&[id], &["whatsapp".into()]);
        }
        assert_eq!(center.ordered().iter().map(|n| n.id).collect::<Vec<_>>(), vec![3,2,1]);
        // Chats read: the badge is gone, only what Windows still has stays.
        center.retain_present(&[], &[]);
        assert!(center.ordered().is_empty());
    }

    #[test]
    fn startup_history_is_read_and_new_items_are_deduplicated() {
        let mut center = NotificationCenter::default();
        assert!(center.receive(vec![notice(1,"Mail")], true).is_none());
        assert_eq!(center.unread_count(), 0);
        assert_eq!(center.receive(vec![notice(1,"Mail"),notice(2,"Mail")], false).unwrap().id, 2);
        center.mark_read(2, 2000);
        assert!(center.receive(vec![notice(2,"Mail")], false).is_none());
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn groups_keep_latest_app_first_and_each_apps_items_newest_first() {
        let mut center = NotificationCenter::default();
        center.receive(vec![notice(1,"Mail"),notice(2,"Chat"),notice(3,"Mail")], false);
        assert_eq!(center.ordered().iter().map(|n| n.id).collect::<Vec<_>>(), vec![3,1,2]);
        assert_eq!(center.app_unread("Mail"), 2);
        center.mark_read(3,3000);
        assert_eq!(center.app_unread("Mail"), 1);
        assert_eq!(center.unread_count(), 2);
    }

    #[test]
    fn clear_does_not_reimport_duplicates_but_reused_id_with_new_time_is_new() {
        let mut center = NotificationCenter::default();
        center.receive(vec![notice(1,"Mail")], false);
        center.clear();
        assert!(center.ordered().is_empty());
        assert!(center.receive(vec![notice(1,"Mail")], false).is_none());
        let mut reused = notice(1,"Mail"); reused.arrival_ms = 5000;
        assert!(center.receive(vec![reused], false).is_some());
    }

    #[test]
    fn reply_typing_backspace_paste_and_limit() {
        let mut reply = Reply::default();
        for c in "oi!\u{1b}".chars() { reply.type_char(c); }
        reply.type_char('\u{8}');
        assert_eq!(reply.text, "oi");
        reply.paste(" tudo\r\nbem");
        assert_eq!(reply.text, "oi tudo bem");
        reply.paste(&"x".repeat(3000));
        assert_eq!(reply.text.chars().count(), 2000);
    }

    #[test]
    fn cleared_items_stay_cleared_after_a_restart() {
        let mut center = NotificationCenter::default();
        center.receive(vec![notice(1,"Mail"),notice(2,"Chat")], false);
        center.clear();
        // A new run starts from the saved watermark and rereads Windows' whole history.
        let mut next = NotificationCenter { cleared_until: center.cleared_until, ..Default::default() };
        next.receive(vec![notice(1,"Mail"),notice(2,"Chat"),notice(3,"Mail")], true);
        assert_eq!(next.ordered().iter().map(|n| n.id).collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn retention_is_bounded_and_oldest_items_leave_first() {
        let mut center = NotificationCenter::default();
        center.receive((1..=105).map(|id| notice(id,"Mail")).collect(), false);
        assert_eq!(center.ordered().len(), 100);
        assert_eq!(center.ordered().last().unwrap().id, 6);
    }

    #[test]
    fn relative_time_handles_future_and_old_timestamps() {
        assert_eq!(notification_age(60_000, 59_000), "agora");
        assert_eq!(notification_age(60_000, 180_000), "2 min");
        assert_eq!(notification_age(60_000, 7_260_000), "2 h");
        assert_eq!(notification_age(0, 172_800_000), "2 d");
    }
}
