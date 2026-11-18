#[cfg(windows)]
#[test]
fn windows_notifications_render_real_color_emoji_and_clip_the_line() {
    use isle_render::notification_text::NotificationText;
    let mut text = NotificationText::new().expect("native Windows text backend");
    let image = text
        .render("\u{1fa79}", 120., 13., false, [245, 241, 235, 255])
        .expect("emoji raster");
    let colored = image
        .pixels()
        .iter()
        .filter(|p| {
            p.alpha() > 80
                && (p.red().abs_diff(p.green()) > 30 || p.green().abs_diff(p.blue()) > 30)
        })
        .count();
    assert!(
        colored > 8,
        "bandage emoji must have color, not a missing glyph box"
    );
    let image = text
        .render(
            "Pessoa \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb} recebeu uma mensagem \u{1f1e7}\u{1f1f7}",
            110.,
            13.,
            false,
            [245, 241, 235, 255],
        )
        .unwrap();
    assert!(image.width() <= 110);
    assert!(image.pixels().iter().any(|p| p.alpha() > 100));
}
