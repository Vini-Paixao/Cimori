use isle_core::state::{ControlTile, IslandState, PillMode, TabPage};
use isle_render::pages::{MediaKey, UiAction, compute_hitboxes};

#[test]
fn navigation_exposes_all_ready_modules_without_shelf_placeholder() {
    let mut state = IslandState::default();
    state.mode = PillMode::Expanded(TabPage::AiHub);
    let boxes = compute_hitboxes(720.0, 600.0, 460.0, 6.0, &state);
    let tabs: Vec<_> = boxes
        .iter()
        .filter_map(|b| {
            if let UiAction::SwitchTab(t) = b.action {
                Some(t)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(tabs.len(), 8);
    assert!(tabs.contains(&TabPage::Tray));
    assert!(!tabs.contains(&TabPage::Shelf));
}

#[test]
fn every_control_and_small_mic_target_remains_inside_panel() {
    let mut state = IslandState::default();
    state.mode = PillMode::Expanded(TabPage::ControlCenter);
    state.audio_available = true;
    state.mic_available = true;
    let boxes = compute_hitboxes(720.0, 600.0, 460.0, 6.0, &state);
    for tile in ControlTile::ALL {
        assert!(boxes.iter().any(|b| b.action == UiAction::Control(tile)));
    }
    let mic = boxes
        .iter()
        .find(|b| b.action == UiAction::ToggleMic)
        .unwrap();
    assert!(
        mic.w <= 40.0 && mic.h >= 32.0,
        "mic should be an icon control"
    );
    for b in boxes {
        assert!(
            b.x >= 60.0 && b.x + b.w <= 660.1 && b.y >= 6.0 && b.y + b.h <= 466.1,
            "{:?}",
            b
        );
    }
}

#[test]
fn media_transport_is_below_artwork_and_play_is_prominent() {
    let mut state = IslandState::default();
    state.mode = PillMode::Expanded(TabPage::Media);
    state.media = Some(Default::default());
    let boxes = compute_hitboxes(720.0, 560.0, 380.0, 6.0, &state);
    let play = boxes
        .iter()
        .find(|b| b.action == UiAction::Media(MediaKey::PlayPause))
        .unwrap();
    let prev = boxes
        .iter()
        .find(|b| b.action == UiAction::Media(MediaKey::Previous))
        .unwrap();
    assert!(play.y >= 230.0);
    assert!(play.w > prev.w);
}

#[test]
fn each_shipped_panel_and_timer_mode_keeps_actions_inside_its_bounds(){
    use isle_core::timers::TimerMode;
    for tab in isle_render::layout::TABS {
        for mode in [TimerMode::Pomodoro,TimerMode::Countdown,TimerMode::Stopwatch,TimerMode::Reminders]{
            let(w,h,_)=isle_render::layout::expanded_size(tab);let mut s=IslandState::default();s.mode=PillMode::Expanded(tab);s.timers.mode=mode;s.audio_available=true;s.mic_available=true;s.media=Some(Default::default());
            for b in compute_hitboxes(720.,w,h,6.,&s){assert!(b.x>=(720.-w)/2.&&b.x+b.w<=(720.+w)/2.+0.1&&b.y>=6.&&b.y+b.h<=6.+h,"{tab:?} {mode:?}: {b:?}");}
        }
    }
}

#[test]
fn removed_artwork_cannot_survive_in_the_render_cache(){
    use isle_core::{artwork::Artwork,spring::PillGeometry};
    use isle_render::renderer::PillRenderer;
    let mut s=IslandState::default();s.mode=PillMode::Expanded(TabPage::Media);
    s.media=Some(isle_core::state::MediaInfo{artwork:Some(std::sync::Arc::new(Artwork::new(1,1,vec![220,30,40,255]).unwrap())),..Default::default()});
    let mut g=PillGeometry::default();g.morph_to(560.,380.,28.);g.width.value=560.;g.height.value=380.;g.corner_radius.value=28.;
    let mut r=PillRenderer::new(720,520);let f=r.render(&g,&s,&Default::default());let index=((164*720+154)*4)as usize;
    assert!(f.bgra_premultiplied[index+2]>200&&f.bgra_premultiplied[index+1]<60);
    s.media.as_mut().unwrap().artwork=None;let f=r.render(&g,&s,&Default::default());
    assert!(f.bgra_premultiplied[index+2]<100,"missing art must show neutral fallback, not old cover");
}

#[test]
fn high_dpi_viewport_fits_and_scrolls_without_losing_primary_actions(){
    let(w,h,_)=isle_render::layout::fit_size(TabPage::ControlCenter,650.,340.);
    assert!(w<=650.&&h<=340.);
    let mut s=IslandState::default();s.mode=PillMode::Expanded(TabPage::ControlCenter);s.audio_available=true;
    s.app_volumes=vec![isle_core::state::AppVolume{id:"app".into(),name:"App".into(),volume_pct:50,muted:false}];
    s.panel_scroll=130.;let boxes=compute_hitboxes(720.,w,h,6.,&s);
    assert!(boxes.iter().any(|b|matches!(b.action,UiAction::AppVolume(0,_))));
    for b in boxes{assert!(b.y>=6.&&b.y+b.h<=h+6.,"{b:?}");}
    s.mode=PillMode::Expanded(TabPage::AiHub);s.chat.open=true;s.panel_scroll=0.;
    let boxes=compute_hitboxes(720.,560.,340.,6.,&s);let send=boxes.iter().find(|b|b.action==UiAction::AiSend).unwrap();
    assert!(send.y+send.h<=346.);
}

#[test]
fn navigation_tabs_sit_in_isles_order_and_fit_any_number() {
    use isle_core::config::Modules;
    use isle_core::state::TabPage;
    let all = Modules::default();
    let every = Modules { clipboard: true, shelf: true, usb: true, downloads: true, ..Modules::default() };
    let three = Modules { ai: true, control: false, agenda: false, weather: true, media: false, notifications: false, timer: false, tray: true, ..Modules::default() };
    for (m, n) in [(all, 8), (every, 12), (three, 3)] {
        for open in [None, Some(TabPage::Notifications), Some(TabPage::AiHub)] {
            let nav = isle_render::layout::navigation(0., 0., 560., &m, open);
            assert_eq!(nav.len(), n);
            for pair in nav.windows(2) {
                assert!(pair[0].x + pair[0].w <= pair[1].x, "overlap");
            }
            assert!(nav.last().map(|b| b.x + b.w <= 540.).unwrap());
        }
    }
    // Music first, Uso (AI usage) last, as isle's Usage tab.
    let nav = isle_render::layout::navigation(0., 0., 560., &all, None);
    assert_eq!(nav.first().unwrap().action, UiAction::SwitchTab(TabPage::Media));
    assert_eq!(nav.last().unwrap().action, UiAction::SwitchTab(TabPage::AiHub));
}

#[test]
fn accent_reaches_the_drawn_pixels() {
    use isle_core::spring::PillGeometry;
    isle_render::set_accent((0, 200, 0));
    let mut s = IslandState::default();
    s.mode = PillMode::Expanded(isle_core::state::TabPage::ControlCenter);
    s.audio_available = true;
    s.volume_pct = 80;
    let (w, h, r) = isle_render::layout::expanded_size(isle_core::state::TabPage::ControlCenter);
    let mut g = PillGeometry::default();
    g.morph_to(w, h, r);
    g.width.value = w;
    g.height.value = h;
    g.corner_radius.value = r;
    let mut renderer = isle_render::renderer::PillRenderer::new(720, 520);
    let fb = renderer.render(&g, &s, &isle_core::config::IsleConfig::default());
    let green = fb.bgra_premultiplied.chunks_exact(4).any(|p| p[1] > 150 && p[0] < 60 && p[2] < 60);
    isle_render::set_accent((241, 139, 99));
    assert!(green, "the volume slider uses the chosen accent");
}

#[test]
fn control_center_buttons_follow_the_settings() {
    use isle_core::state::{ControlTile, TabPage};
    use isle_render::pages::UiAction;
    let mut s = IslandState::default();
    s.mode = PillMode::Expanded(TabPage::ControlCenter);
    s.audio_available = true;
    s.audio_outputs = ["a", "b"].map(|id| isle_core::state::AudioOutput { id: id.into(), name: id.into(), is_default: false }).to_vec();
    let (w, h, _) = isle_render::layout::expanded_size(TabPage::ControlCenter);
    let full = compute_hitboxes(720., w, h, 6., &s);
    s.control_tiles = vec![ControlTile::Explorer, ControlTile::Wifi];
    s.control_outputs = false;
    s.control_mixer = false;
    let few = compute_hitboxes(720., w, h, 6., &s);
    let tiles: Vec<_> = few.iter().filter_map(|b| match b.action { UiAction::Control(t) => Some((t, b.h)), _ => None }).collect();
    assert_eq!(tiles, vec![(ControlTile::Explorer, 58.), (ControlTile::Wifi, 58.)], "the first two are the big ones");
    let volume_y = |boxes: &[isle_render::pages::HitBox]| boxes.iter().find(|b| matches!(b.action, UiAction::SetVolume(_))).unwrap().y;
    assert!(volume_y(&few) < volume_y(&full), "the volume moves up when there are fewer buttons");
    assert!(!few.iter().any(|b| matches!(b.action, UiAction::SetOutput(_) | UiAction::OpenMixer)));
}

#[test]
fn phase8_navigation_and_download_action_follow_module_switches() {
    use isle_render::phase8_view::Phase8Action;
    let mut s = IslandState::default();
    assert_eq!(isle_render::layout::visible_tabs(&s.modules).len(), 8);
    s.modules.clipboard = true;
    s.modules.shelf = true;
    s.modules.usb = true;
    s.modules.downloads = true;
    s.mode = PillMode::Expanded(TabPage::Downloads);
    let hits = compute_hitboxes(720., 560., 450., 6., &s);
    assert_eq!(hits.iter().filter(|b| matches!(b.action, UiAction::SwitchTab(_))).count(), 12);
    assert!(hits.iter().any(|b| b.action == UiAction::Phase8(Phase8Action::OpenDownloads)));
}

#[test]
fn compact_media_controls_share_the_slots_for_full_and_play_only_modes() {
    let mut state = IslandState::default();
    state.ui.media_controls = true;
    state.bar.push(isle_core::config::BarItem::Media);
    state.media = Some(isle_core::state::MediaInfo { title: "Uma música".into(), ..Default::default() });
    let controls = |s: &IslandState| {
        let width = isle_render::pages::compact_tray_width(s);
        compute_hitboxes(720., width, 34., 6., s).into_iter().filter_map(|hit| {
            if let UiAction::Media(key) = hit.action { Some(key) } else { None }
        }).collect::<Vec<_>>()
    };
    assert_eq!(controls(&state), [MediaKey::Previous, MediaKey::PlayPause, MediaKey::Next]);
    state.ui.media_play_only = true;
    assert_eq!(controls(&state), [MediaKey::PlayPause]);
    state.modules.media = false;
    assert!(controls(&state).is_empty());
}

#[test]
fn chosen_output_and_tiles_follow_the_accent() {
    use isle_core::spring::PillGeometry;
    for scale in [1.0, 1.5] {
        isle_render::set_accent((0, 200, 0));
        let mut s = IslandState::default();
        s.mode = PillMode::Expanded(TabPage::ControlCenter);
        s.audio_available = true;
        s.audio_outputs = [("a", true), ("b", false)]
            .map(|(id, d)| isle_core::state::AudioOutput { id: id.into(), name: "Alto-falantes".into(), is_default: d })
            .to_vec();
        let (w, h, r) = isle_render::layout::expanded_size(TabPage::ControlCenter);
        let mut g = PillGeometry::default();
        g.morph_to(w, h, r);
        g.width.value = w;
        g.height.value = h;
        g.corner_radius.value = r;
        let cfg = isle_core::config::IsleConfig::default();
        let mut renderer = isle_render::renderer::PillRenderer::with_scale(720, 520, scale);
        let fb = renderer.render(&g, &s, &cfg);
        let px = |x: f32, y: f32| {
            let i = (((y * scale) as u32 * fb.width + (x * scale) as u32) * 4) as usize;
            (fb.bgra_premultiplied[i + 2], fb.bgra_premultiplied[i + 1])
        };
        let boxes = compute_hitboxes(720., w, h, cfg.gap_px as f32, &s);
        let chip = boxes.iter().find(|b| b.action == UiAction::SetOutput(0)).unwrap();
        let other = boxes.iter().find(|b| b.action == UiAction::SetOutput(1)).unwrap();
        let (cr, cg) = px(chip.x + 3., chip.y + chip.h / 2.);
        let (or, og) = px(other.x + 3., other.y + other.h / 2.);
        let apricot = fb.bgra_premultiplied.chunks_exact(4).filter(|p| p[2] > 200 && (110..170).contains(&p[1]) && p[0] < 130).count();
        isle_render::set_accent((241, 139, 99));
        assert!(cg > cr && cg > og, "the chosen output is green-tinted at {scale}: {cr},{cg} vs {or},{og}");
        assert_eq!(apricot, 0, "no default-accent pixel left at {scale}");
    }
}

#[test]
fn the_open_tab_says_its_name_on_a_pill_and_the_star_sits_before_the_date() {
    use isle_core::spring::PillGeometry;
    use isle_core::state::TabPage;
    for scale in [1.0f32, 1.5] {
        let mut s = IslandState::default();
        s.mode = PillMode::Expanded(TabPage::Media);
        s.date_text = "sex 13 nov".into();
        let (w, h, r) = isle_render::layout::expanded_size(TabPage::Media);
        let mut g = PillGeometry::default();
        g.morph_to(w, h, r);
        g.width.value = w;
        g.height.value = h;
        g.corner_radius.value = r;
        let mut renderer = isle_render::renderer::PillRenderer::with_scale(720, 520, scale);
        let fb = renderer.render(&g, &s, &isle_core::config::IsleConfig::default());
        let boxes = isle_render::pages::compute_hitboxes(720.0, w, h, 6.0, &s);
        let tab = |t| boxes.iter().find(|b| b.action == UiAction::SwitchTab(t)).unwrap();
        let (music, bell) = (tab(TabPage::Media), tab(TabPage::Notifications));
        assert!(music.w > bell.w + 30., "the open tab is wider: its name shows");
        // Ink right of the icon, inside the pill: the name "Música".
        let px = |v: f32| (v * scale).round() as i32;
        let ink = (px(music.x + 30.)..px(music.x + music.w - 4.)).flat_map(|x| (px(music.y + 4.)..px(music.y + music.h - 4.)).map(move |y| (x, y)))
            .filter(|&(x, y)| fb.bgra_premultiplied[((y as u32 * fb.width + x as u32) * 4) as usize + 1] > 180).count();
        assert!(ink > 10, "the name is drawn @{scale}");
        let star = boxes.iter().find(|b| b.action == UiAction::AiOpen).expect("the ✦");
        let gear = boxes.iter().find(|b| b.action == UiAction::OpenSettings).unwrap();
        assert!(star.x + star.w < gear.x && star.y < 40., "right side of the header: ✦ … ⚙");
    }
}

