//! Read-only Shell icon diagnostic. No notification database or private messages are read.
#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use windows::Win32::System::Com::*;
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    let _com = Com;
    let id = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("expected app AUMID"))?;
    let path = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("expected output PNG"))?;
    let art = isle_platform_win::notification_icons::load(&id)?;
    let mut rgba = art.rgba;
    for p in rgba.chunks_exact_mut(4) {
        let a = p[3] as u16;
        for c in &mut p[..3] {
            *c = (*c as u16 * a / 255) as u8;
        }
    }
    let pix = tiny_skia::Pixmap::from_vec(
        rgba,
        tiny_skia::IntSize::from_wh(art.width, art.height).unwrap(),
    )
    .unwrap();
    pix.save_png(&path)?;
    println!("Shell icon: {id} | {}x{} | {path}", art.width, art.height);
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("Windows Shell required");
}
