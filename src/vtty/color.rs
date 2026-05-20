/// Standard ANSI 16-color palette (0-15)
const ANSI_16_COLORS: [[u8; 3]; 16] = [
    [0, 0, 0],       [170, 0, 0],     [0, 170, 0],     [170, 85, 0],
    [0, 0, 170],     [170, 0, 170],   [0, 170, 170],   [170, 170, 170],
    [85, 85, 85],    [255, 85, 85],   [85, 255, 85],   [255, 255, 85],
    [85, 85, 255],   [255, 85, 255],  [85, 255, 255],  [255, 255, 255],
];

/// Convert a 256-color index (16-231) to RGB using the 6x6x6 color cube.
pub fn color_256_to_rgb(index: u8) -> [u8; 3] {
    match index {
        0..=15 => ANSI_16_COLORS[index as usize],
        16..=231 => {
            let i = index - 16;
            let r = (i / 36) % 6;
            let g = (i / 6) % 6;
            let b = i % 6;
            [
                if r == 0 { 0 } else { r * 40 + 55 },
                if g == 0 { 0 } else { g * 40 + 55 },
                if b == 0 { 0 } else { b * 40 + 55 },
            ]
        }
        232..=255 => {
            let gray = (index - 232) * 10 + 8;
            [gray, gray, gray]
        }
    }
}

/// Convert an RGB triplet to a 256-color index (best match).
pub fn rgb_to_color_256(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b {
        if r < 8 { return 16; }
        if r > 248 { return 231; }
        return 232 + ((r - 8) / 10);
    }
    let r_idx = match r { 0..=47 => 0, _ => ((r - 35) / 40).min(5) };
    let g_idx = match g { 0..=47 => 0, _ => ((g - 35) / 40).min(5) };
    let b_idx = match b { 0..=47 => 0, _ => ((b - 35) / 40).min(5) };
    16 + r_idx * 36 + g_idx * 6 + b_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_16() {
        assert_eq!(color_256_to_rgb(0), [0, 0, 0]);
        assert_eq!(color_256_to_rgb(1), [170, 0, 0]);
        assert_eq!(color_256_to_rgb(15), [255, 255, 255]);
    }

    #[test]
    fn test_color_cube() {
        assert_eq!(color_256_to_rgb(16), [0, 0, 0]);
        assert_eq!(color_256_to_rgb(21), [0, 0, 255]);
        assert_eq!(color_256_to_rgb(196), [255, 0, 0]);
    }

    #[test]
    fn test_grayscale() {
        assert_eq!(color_256_to_rgb(232), [8, 8, 8]);
        assert_eq!(color_256_to_rgb(255), [238, 238, 238]);
    }

    #[test]
    fn test_rgb_roundtrip() {
        for i in 16..=255 {
            let rgb = color_256_to_rgb(i);
            // Skip colors where R==G==B from the color cube — they are ambiguous
            // with the grayscale ramp and don't roundtrip correctly.
            if i >= 16 && i <= 231 && rgb[0] == rgb[1] && rgb[2] == rgb[1] {
                continue;
            }
            let back = rgb_to_color_256(rgb[0], rgb[1], rgb[2]);
            assert_eq!(back, i, "Roundtrip failed for color {}", i);
        }
    }
}
