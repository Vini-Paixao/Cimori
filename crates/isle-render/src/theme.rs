use tiny_skia::Color;
pub(crate) fn white() -> Color {
    Color::from_rgba8(245, 241, 235, 255)
}
pub(crate) fn muted() -> Color {
    Color::from_rgba8(167, 173, 183, 255)
}
pub(crate) fn surface() -> Color {
    Color::from_rgba8(42, 46, 54, 255)
}
thread_local! {
    /// The settings' accent; per thread because only the UI thread draws (and tests draw in parallel).
    static ACCENT: std::cell::Cell<(u8, u8, u8)> = const { std::cell::Cell::new((241, 139, 99)) };
}
/// The accent this thread draws with (`IsleConfig::accent_rgb`).
pub fn set_accent(rgb: (u8, u8, u8)) {
    ACCENT.with(|a| a.set(rgb));
}
pub(crate) fn accent() -> Color {
    accent_alpha(255)
}
/// The accent at `alpha` (0..=255); every accent tint derives from the chosen accent.
pub(crate) fn accent_alpha(alpha: u8) -> Color {
    let (r, g, b) = ACCENT.with(|a| a.get());
    Color::from_rgba8(r, g, b, alpha)
}
/// The accent lightened toward white (`t` of the way), e.g. the user's lines in the chat.
pub(crate) fn accent_light(t: f32) -> Color {
    let (r, g, b) = ACCENT.with(|a| a.get());
    let mix = |c: u8| (c as f32 + (255. - c as f32) * t).round() as u8;
    Color::from_rgba8(mix(r), mix(g), mix(b), 255)
}
pub(crate) fn ink() -> Color {
    Color::from_rgba8(23, 25, 29, 255)
}
pub(crate) fn red() -> Color {
    Color::from_rgba8(246, 145, 132, 255)
}
pub(crate) fn orange() -> Color {
    Color::from_rgba8(235, 188, 111, 255)
}
pub(crate) fn line() -> Color {
    Color::from_rgba8(245, 241, 235, 17)
}
/// Selected backgrounds (tab, chosen output, active tiles): the accent at ~10%.
pub(crate) fn tint() -> Color {
    accent_alpha(26)
}
