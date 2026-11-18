use isle_render::icon_images::raster_picture;

#[test]
fn svg_shapes_and_current_color_are_tinted_without_losing_alpha() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><path fill="currentColor" d="M0 0h24v12H0z"/></svg>"##;
    let p = raster_picture(svg, 36, [10, 200, 30, 128], true).unwrap();
    assert_eq!((p.width(), p.height()), (36, 36));
    let center = p.pixel(18, 18).unwrap();
    assert_eq!((center.red(), center.green(), center.blue(), center.alpha()), (5, 100, 15, 128));
    assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0);
    assert!(raster_picture(b"invalid", 24, [255;4], true).is_none());
}

#[cfg(windows)]
#[test]
fn native_segoe_glyphs_have_ink_and_keep_the_requested_tint_at_both_scales() {
    let mut text=isle_render::notification_text::NotificationText::new().unwrap();
    for size in [20.,30.] {
        for icon in isle_core::icons::Icon::ALL {
        let p=text.render_glyph(icon.info().glyph,size,[0,180,0,255]).unwrap();
        assert!(p.width()>0 && p.height()>0);
        assert!(p.data().chunks_exact(4).any(|c|c[3]>0));
        assert!(p.data().chunks_exact(4).all(|c|c[0]==0 && c[2]==0));
        }
    }
}

#[test]
fn custom_png_keeps_its_colors_and_is_scaled_to_physical_pixels() {
    let mut original = tiny_skia::Pixmap::new(2, 1).unwrap();
    original.fill(tiny_skia::Color::from_rgba8(10, 200, 30, 255));
    let p = raster_picture(&original.encode_png().unwrap(), 30, [255, 0, 0, 255], false).unwrap();
    assert_eq!(p.pixel(15, 15).unwrap().green(), 200);
    assert_eq!(p.pixel(15, 0).unwrap().alpha(), 0);
}

#[test]
fn pictures_without_visible_ink_use_the_windows_fallback() {
    let empty=br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"/>"#;
    assert!(raster_picture(empty,24,[255;4],true).is_none());
    let png=tiny_skia::Pixmap::new(16,16).unwrap().encode_png().unwrap();
    assert!(raster_picture(&png,24,[255;4],false).is_none());
}
