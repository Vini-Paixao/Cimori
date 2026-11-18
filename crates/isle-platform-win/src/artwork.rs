//! WIC decoding on a COM-initialized worker, never on the UI thread.
#[cfg(windows)]
pub fn decode(bytes: &[u8]) -> anyhow::Result<isle_core::artwork::Artwork> {
    use windows::Win32::{
        Graphics::Imaging::*,
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
        UI::Shell::SHCreateMemStream,
    };
    anyhow::ensure!(
        isle_core::artwork::valid_encoded_size(bytes.len() as u64),
        "invalid artwork byte count"
    );
    unsafe {
        let stream = SHCreateMemStream(Some(bytes))
            .ok_or_else(|| anyhow::anyhow!("image stream unavailable"))?;
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnLoad,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        let (dw, dh) = isle_core::artwork::normalized_size(w, h)
            .ok_or_else(|| anyhow::anyhow!("invalid artwork dimensions"))?;
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(&frame, dw, dh, WICBitmapInterpolationModeFant)?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None,
            0.,
            WICBitmapPaletteTypeCustom,
        )?;
        let mut rgba = vec![0u8; dw as usize * dh as usize * 4];
        converter.CopyPixels(std::ptr::null(), dw * 4, &mut rgba)?;
        isle_core::artwork::Artwork::new(dw, dh, rgba)
            .ok_or_else(|| anyhow::anyhow!("invalid normalized artwork"))
    }
}
