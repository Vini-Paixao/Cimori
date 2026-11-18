#[test]
fn toast_preserves_emoji_surrogates_joiners_and_variations() {
    let payload = "<toast><visual><binding><text>\u{1fa79} \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}</text><text>Mensagem de voz \u{1f1e7}\u{1f1f7} &#x2764;&#xFE0F;</text></binding></visual></toast>";
    let (title, body) = isle_platform_win::notifications::parse_toast(payload.as_bytes()).unwrap();
    assert_eq!(title, "\u{1fa79} \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}");
    assert_eq!(body, "Mensagem de voz \u{1f1e7}\u{1f1f7} \u{2764}\u{fe0f}");
}
