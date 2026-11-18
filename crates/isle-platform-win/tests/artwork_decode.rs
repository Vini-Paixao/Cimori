#[cfg(windows)]
#[test]
fn wic_accepts_png_and_jpeg_but_rejects_bad_streams() {
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED).unwrap();
    }
    for bytes in [
        include_bytes!("fixtures/artwork.png").as_slice(),
        include_bytes!("fixtures/artwork.jpg").as_slice(),
    ] {
        let a = isle_platform_win::artwork::decode(bytes).unwrap();
        assert_eq!((a.width, a.height), (96, 48));
        assert_eq!(a.rgba.len(), 96 * 48 * 4);
        assert!(a.rgba[0] > 160 && a.rgba[1] < 100 && a.rgba[3] == 255);
    }
    assert!(isle_platform_win::artwork::decode(b"invalid image").is_err());
    assert!(isle_platform_win::artwork::decode(&[]).is_err());
    unsafe {
        RoUninitialize();
    }
}
