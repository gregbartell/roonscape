/// Pack image rows for native RGBA uploads, omitting row padding and supplying
/// opaque alpha for RGB images. The final row need not include trailing padding.
pub fn pixbuf_rgba(image: &gdk_pixbuf::Pixbuf) -> Vec<u8> {
    let bytes = image.read_pixel_bytes();
    let channels = image.n_channels() as usize;
    let width = image.width() as usize;
    let mut rgba = Vec::with_capacity(width * image.height() as usize * 4);
    for row in 0..image.height() as usize {
        let start = row * image.rowstride() as usize;
        for pixel in bytes[start..start + width * channels].chunks_exact(channels) {
            rgba.extend_from_slice(&[
                pixel[0],
                pixel[1],
                pixel[2],
                if channels == 4 { pixel[3] } else { 255 },
            ]);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::pixbuf_rgba;
    use gdk_pixbuf::{Colorspace, Pixbuf};
    use gtk::glib::Bytes;

    #[test]
    fn rgb_rows_skip_padding_and_add_opaque_alpha() {
        let image = Pixbuf::from_bytes(
            &Bytes::from_owned([1, 2, 3, 4, 5, 6, 99, 99, 7, 8, 9, 10, 11, 12]),
            Colorspace::Rgb,
            false,
            8,
            2,
            2,
            8,
        );
        assert_eq!(
            pixbuf_rgba(&image),
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }

    #[test]
    fn rgba_rows_preserve_alpha_and_skip_padding() {
        let image = Pixbuf::from_bytes(
            &Bytes::from_owned([1, 2, 3, 4, 99, 99, 99, 99, 5, 6, 7, 8]),
            Colorspace::Rgb,
            true,
            8,
            1,
            2,
            8,
        );
        assert_eq!(pixbuf_rgba(&image), [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
