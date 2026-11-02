//! The pairing QR code, as pixels for the app to show.

use qrcode::{Color, EcLevel, QrCode};

/// Modules of white border around the code, as scanners expect.
const QUIET: usize = 4;

/// `text` as a QR code: its size in pixels and RGBA pixels, dark modules on
/// white, each module `scale` pixels wide. `None` if the text is too long.
pub fn rgba(text: &str, scale: usize) -> Option<(usize, Vec<u8>)> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).ok()?;
    let modules = code.width();
    let colors = code.to_colors();
    let scale = scale.max(1);
    let side = (modules + 2 * QUIET) * scale;
    let mut px = vec![255u8; side * side * 4];
    for (i, c) in colors.iter().enumerate() {
        if *c != Color::Dark {
            continue;
        }
        let (mx, my) = (i % modules + QUIET, i / modules + QUIET);
        for y in my * scale..(my + 1) * scale {
            for x in mx * scale..(mx + 1) * scale {
                let p = (y * side + x) * 4;
                px[p..p + 3].copy_from_slice(&[0x10, 0x11, 0x12]);
            }
        }
    }
    Some((side, px))
}

#[cfg(test)]
mod tests {
    #[test]
    fn draws_a_code() {
        let (side, px) = super::rgba(
            "http://192.168.1.20:7477/#pair=0123456789abcdef0123456789abcdef",
            4,
        )
        .unwrap();
        assert_eq!(px.len(), side * side * 4);
        // White border, dark finder pattern in the corner.
        assert_eq!(px[0], 255);
        let corner = (super::QUIET * 4 * side + super::QUIET * 4) * 4;
        assert_eq!(px[corner], 0x10);
    }
}
