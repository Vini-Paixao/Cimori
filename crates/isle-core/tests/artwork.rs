use isle_core::artwork::{Artwork, normalized_size, valid_encoded_size};
#[test]
fn artwork_enforces_stream_and_pixel_limits_before_allocating() {
    assert!(!valid_encoded_size(0));
    assert!(valid_encoded_size(8 * 1024 * 1024));
    assert!(!valid_encoded_size(8 * 1024 * 1024 + 1));
    assert_eq!(normalized_size(1200, 600), Some((512, 256)));
    assert_eq!(normalized_size(300, 600), Some((256, 512)));
    assert_eq!(normalized_size(0, 32), None);
    assert_eq!(normalized_size(4097, 1), None);
    assert!(Artwork::new(2, 2, vec![0; 15]).is_none());
    assert!(Artwork::new(2, 2, vec![255; 16]).is_some());
    assert!(Artwork::new(513, 1, vec![0; 2052]).is_none());
}
