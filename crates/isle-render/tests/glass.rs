use isle_core::config::IsleConfig;
use isle_render::glass::{tone, tone_graph, evaluate, glass_path};

#[test]
fn old_configs_leave_glass_off_and_bright_backdrops_are_held_deeper() {
    let cfg: IsleConfig=serde_json::from_str("{}").unwrap();
    assert!(!cfg.liquid_glass);
    for (input,expected) in [(0.,0.03),(0.2,0.158),(1.,0.3372)] {
        assert!((tone([input;3],false)[0]-expected).abs()<0.0001);
    }
}

#[test]
fn glass_is_clipped_translucent_and_retains_the_accent() {
    use isle_core::{state::{IslandState,PillMode,TabPage},spring::PillGeometry};
    use isle_render::renderer::PillRenderer;
    for notch in [false,true] {for scale in [1.,1.5] {
        let mut cfg=IsleConfig::default();cfg.liquid_glass=true;
        if notch {cfg.ui.pill_style=isle_core::config::PillStyle::Notch;}
        let mut geom=PillGeometry::default();geom.morph_to(400.,260.,28.);
        geom.width.value=400.;geom.height.value=260.;geom.corner_radius.value=28.;
        let mut state=IslandState::default();state.mode=PillMode::Expanded(TabPage::ControlCenter);
        state.control_editing=true; // "Concluído" in the accent (the open tab is white, as isle's)
        isle_render::set_accent((0,200,20));
        let mut renderer=PillRenderer::with_scale(720,520,scale);
        let fb=renderer.render(&geom,&state,&cfg);
        let at=|x:f32,y:f32|{let i=((y*scale) as u32*fb.width+(x*scale) as u32) as usize*4;&fb.bgra_premultiplied[i..i+4]};
        assert_eq!(at(0.,0.)[3],0);
        assert!(at(360.,250.)[3]<240,"body remains translucent");
        assert!(fb.bgra_premultiplied.chunks_exact(4).any(|p|p[1]>100 && u16::from(p[1])>u16::from(p[2])*2));
    }}
}

#[test]
fn composition_graph_matches_isle_shader_without_sampling_the_screen() {
    for expanded in [false,true] {
        let graph=tone_graph(expanded);
        for r in 0..11 { for g in 0..11 { for b in 0..11 {
            let rgb=[r as f32/10.,g as f32/10.,b as f32/10.];
            let expected=tone(rgb,expanded); let actual=evaluate(&graph,[rgb[0],rgb[1],rgb[2],1.]);
            for i in 0..3 { assert!((expected[i]-actual[i]).abs()<0.0001,"{rgb:?}: {actual:?} != {expected:?}"); }
            assert!((actual[3]-1.).abs()<0.0001);
        } } }
    }
}

#[test]
fn native_backdrop_and_renderer_use_identical_pill_and_notch_paths() {
    for notch in [false,true] { for scale in [1.,1.5] {
        let path=glass_path(720.,200.,34.,17.,6.,notch,75,scale).unwrap();
        let mut mask=tiny_skia::Mask::new((720.*scale) as u32,(80.*scale) as u32).unwrap();
        mask.fill_path(&path,tiny_skia::FillRule::Winding,true,tiny_skia::Transform::identity());
        let point=|x:f32,y:f32|mask.data()[(y*scale) as usize*mask.width() as usize+(x*scale) as usize];
        assert_eq!(point(360.,20.),255); assert_eq!(point(0.,20.),0);
        assert_eq!(point(360.,0.),if notch {255}else{0});
    } }
}
