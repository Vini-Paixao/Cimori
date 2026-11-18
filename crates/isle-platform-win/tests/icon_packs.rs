use isle_core::icons::{Icon, IconPack};
use isle_platform_win::icon_packs::{load_folder, pack_path};

#[test]
fn folder_names_and_codes_are_read_without_network() {
    let root = std::env::temp_dir().join(format!("isle-icons-{}",std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("E713.svg"), b"<svg/>").unwrap();
    std::fs::write(root.join("microphone.png"), b"png fixture").unwrap();
    let images = load_folder(&root,IconPack::Folder,root.to_string_lossy().into());
    assert_eq!(images.pictures.get(&Icon::Gear.info().glyph).unwrap(), b"<svg/>");
    assert!(images.pictures.contains_key(&Icon::Mic.info().glyph));
    assert!(!images.pictures.contains_key(&Icon::Sun.info().glyph));
    assert!(pack_path(&root,IconPack::Windows).is_none());
    assert_eq!(pack_path(&root,IconPack::Lucide).unwrap(),root.join("lucide-1.40.0"));
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn cached_pack_and_404_markers_are_never_requested_again() {
    let root=std::env::temp_dir().join(format!("isle-icons-cached-{}",std::process::id()));
    let path=pack_path(&root,IconPack::Lucide).unwrap(); std::fs::create_dir_all(&path).unwrap();
    for icon in Icon::ALL { if let Some(name)=icon.info().names[2] {std::fs::write(path.join(format!("{name}.none")),[]).unwrap();} }
    std::fs::write(path.join("settings.svg"),b"<svg/>").unwrap();
    let images=isle_platform_win::icon_packs::prepare(&root,IconPack::Lucide,String::new(),|done,total|assert_eq!((done,total),(0,0))).await.unwrap();
    assert!(images.pictures.contains_key(&Icon::Gear.info().glyph));
    std::fs::remove_dir_all(root).unwrap();
}
