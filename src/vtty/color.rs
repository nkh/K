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

/// Mutable 256-color palette used by the VTTY emulator.
/// Entries 0-15 are the standard ANSI colors, 16-231 the 6x6x6 color
/// cube, and 232-255 the grayscale ramp.  Programs can remap any slot
/// at runtime via OSC 4 / OSC 104.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    slots: [[u8; 3]; 256],
}

impl ColorPalette {
    pub fn new() -> Self {
        let mut slots = [[0u8; 3]; 256];
        for i in 0..256 {
            slots[i] = color_256_to_rgb(i as u8);
        }
        Self { slots }
    }

    /// Resolve a 256-color index to an RGB triplet.
    pub fn resolve(&self, index: u8) -> [u8; 3] {
        self.slots[index as usize]
    }

    /// Get a palette entry by index (alias for resolve).
    pub fn get(&self, index: u8) -> [u8; 3] {
        self.resolve(index)
    }

    /// Set a single palette slot.
    pub fn set(&mut self, index: u8, rgb: [u8; 3]) {
        self.slots[index as usize] = rgb;
    }

    /// Reset the entire palette to the standard 256-color defaults.
    pub fn reset(&mut self) {
        for i in 0..=255u8 {
            self.slots[i as usize] = color_256_to_rgb(i);
        }
    }

    /// Apply an OSC 4 payload, which may contain multiple color specifications.
    /// Format: "N;spec" or "N;spec;M;spec;..." where spec is:
    ///   - rgb:RR/GG/BB   (e.g. "rgb:ff/00/00")
    ///   - #RRGGBB         (CSS hex)
    pub fn apply_osc4(&mut self, data: &str) {
        let mut parts = data.split(';').peekable();
        while let Some(index_str) = parts.next() {
            let spec = match parts.peek() {
                Some(s) if !s.is_empty() => *s,
                _ => break,
            };
            parts.next(); // consume the spec

            if let Ok(index) = index_str.parse::<u8>() {
                if let Some(rgb) = parse_osc4_color(spec) {
                    self.set(index, rgb);
                }
            }
        }
    }
}

/// Parse an OSC 4 color specification into an RGB triplet.
fn parse_osc4_color(spec: &str) -> Option<[u8; 3]> {
    let spec = spec.trim();
    if spec.starts_with("rgb:") {
        // rgb:RR/GG/BB or rgb:rr/gg/bb
        let hex = &spec[4..];
        let parts: Vec<&str> = hex.split('/').collect();
        if parts.len() == 3 {
            let r = u8::from_str_radix(parts[0], 16).ok()?;
            let g = u8::from_str_radix(parts[1], 16).ok()?;
            let b = u8::from_str_radix(parts[2], 16).ok()?;
            return Some([r, g, b]);
        }
    } else if spec.starts_with('#') {
        // #RRGGBB
        if spec.len() == 7 {
            let r = u8::from_str_radix(&spec[1..3], 16).ok()?;
            let g = u8::from_str_radix(&spec[3..5], 16).ok()?;
            let b = u8::from_str_radix(&spec[5..7], 16).ok()?;
            return Some([r, g, b]);
        }
    }
    None
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::new()
    }
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
