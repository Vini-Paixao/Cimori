use isle_core::{config::IsleConfig, icons::{Icon, IconPack}};

#[test]
fn every_icon_has_the_upstream_glyph_and_pinned_pack_names() {
    assert_eq!(Icon::ALL.len(), 65);
    assert_eq!(Icon::Gear.info().glyph, 0xE713);
    assert_eq!(Icon::Gear.info().name, "settings");
    assert_eq!(Icon::Send.info().glyph,0xE724);
    for icon in Icon::ALL {
        assert!(char::from_u32(icon.info().glyph).is_some());
        for pack in IconPack::ONLINE {
            if let Some(url) = pack.url(icon) {
                assert!(url.starts_with("https://cdn.jsdelivr.net/npm/"));
                assert!(url.ends_with(".svg"));
                assert!(!url.contains("latest"));
            }
        }
    }
    assert_eq!(IconPack::Lucide.url(Icon::Gear).unwrap(), "https://cdn.jsdelivr.net/npm/lucide-static@1.40.0/icons/settings.svg");
    assert!(IconPack::Windows.url(Icon::Gear).is_none());
    assert!(IconPack::Folder.url(Icon::Gear).is_none());
}

#[test]
fn old_configs_keep_native_icons_and_unpinned_arrow() {
    let c: IsleConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(c.icon_pack, IconPack::Windows);
    assert!(c.icon_folder.is_empty());
    assert!(!c.ui.arrow_pins);
    let json = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<IsleConfig>(&json).unwrap(), c);
}
