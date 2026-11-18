//! Read-only diagnostic for the media snapshot and artwork pipeline.
fn main() -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _watch = isle_platform_win::media::watch(move |s| {
        let _ = tx.send(s);
    })?;
    let status = rx.recv_timeout(std::time::Duration::from_secs(6))?;
    if let Some(s) = status {
        println!(
            "Session: {} | playing: {} | artwork: {:?}",
            s.app_id,
            s.playing,
            s.artwork.as_ref().map(|a| (a.width, a.height))
        );
        if let (Some(a), Some(path)) = (s.artwork, std::env::args().nth(1)) {
            let mut rgba = a.rgba.clone();
            for px in rgba.chunks_exact_mut(4) {
                let alpha = px[3] as u16;
                for c in &mut px[..3] {
                    *c = (*c as u16 * alpha / 255) as u8;
                }
            }
            let p = tiny_skia::Pixmap::from_vec(
                rgba,
                tiny_skia::IntSize::from_wh(a.width, a.height).unwrap(),
            )
            .unwrap();
            p.save_png(&path)?;
            println!("Saved {path}");
        }
    } else {
        println!("No active media session");
    }
    Ok(())
}
