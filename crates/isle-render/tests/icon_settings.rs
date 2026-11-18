use isle_core::{config::IsleConfig,prefs::SettingsTab};
use isle_render::{settings::{self,SettingsView,SettingsAction},text::Text};

#[test]
fn every_pack_can_be_selected_in_wide_and_narrow_settings() {
    for (w,h) in [(1180.,760.),(600.,500.)] {
        let cfg=IsleConfig::default(); let text=Text::new();
        let mut found=std::collections::HashSet::new();
        for scroll in (0..1500).step_by(20) {
            let view=SettingsView {tab:SettingsTab::Icons,scroll:scroll as f32,..Default::default()};
            let layout=settings::layout(&view,&cfg,&text,w,h);
            for hit in &layout.hits {
                if let SettingsAction::Choose("icon_pack",i)=hit.action {
                    assert_eq!(settings::hit(&layout,hit.x+hit.w/2.,hit.y+hit.h/2.),Some(hit.action.clone()));
                    found.insert(i);
                }
            }
        }
        assert_eq!(found.len(),11,"{w}px");
    }
}
