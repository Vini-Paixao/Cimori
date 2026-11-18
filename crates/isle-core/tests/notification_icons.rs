use isle_core::{
    artwork::Artwork,
    notifications::{Notification, NotificationCenter},
};
#[test]
fn late_app_icon_updates_matching_items_without_new_unread_notifications() {
    let mut c = NotificationCenter::default();
    c.receive(
        vec![Notification {
            id: 1,
            app_id: "WhatsApp!App".into(),
            title: "\u{1fa79}".into(),
            arrival_ms: 1000,
            ..Default::default()
        }],
        false,
    );
    c.mark_read(1, 1000);
    let icon = std::sync::Arc::new(Artwork::new(1, 1, vec![20, 200, 70, 255]).unwrap());
    c.set_app_icon("whatsapp!app", icon.clone());
    let items = c.ordered();
    assert_eq!(items[0].title, "\u{1fa79}");
    assert!(!items[0].unread);
    assert!(std::sync::Arc::ptr_eq(
        items[0].app_icon.as_ref().unwrap(),
        &icon
    ));
    assert_eq!(c.unread_count(), 0);
}
