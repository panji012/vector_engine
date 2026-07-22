use vector_core::{VectorCoreError, VectorDocument};

pub struct LuminanceEngine;

impl LuminanceEngine {
    /// Adjusts the luminance / brightness of colors in a `VectorDocument` using a configurable multiplier `factor`.
    ///
    /// - `factor > 1.0` brightens colors.
    /// - `factor < 1.0` darkens colors.
    pub fn adjust_document_luminance(
        doc: &VectorDocument,
        factor: f64,
    ) -> Result<VectorDocument, VectorCoreError> {
        if factor < 0.0 {
            return Err(VectorCoreError::InvalidGeometry("Luminance factor must be non-negative".to_string()));
        }

        let mut adjusted_doc = doc.clone();

        for path in &mut adjusted_doc.paths {
            if let Some(ref fill_hex) = path.fill {
                if let Ok(new_hex) = adjust_hex_luminance(fill_hex, factor) {
                    path.fill = Some(new_hex);
                }
            }
            if let Some(ref stroke_hex) = path.stroke {
                if let Ok(new_hex) = adjust_hex_luminance(stroke_hex, factor) {
                    path.stroke = Some(new_hex);
                }
            }
        }

        Ok(adjusted_doc)
    }
}

/// Adjusts a HEX color string's luminance by multiplying lightness in HSL space.
pub fn adjust_hex_luminance(hex: &str, factor: f64) -> Result<String, VectorCoreError> {
    let (r, g, b) = parse_hex_color(hex)?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let new_l = (l * factor).clamp(0.0, 1.0);
    let (new_r, new_g, new_b) = hsl_to_rgb(h, s, new_l);
    Ok(format_hex_color(new_r, new_g, new_b))
}

fn parse_hex_color(hex: &str) -> Result<(f64, f64, f64), VectorCoreError> {
    let clean = hex.trim_start_matches('#');
    let (r_u8, g_u8, b_u8) = match clean.len() {
        6 => (
            u8::from_str_radix(&clean[0..2], 16).unwrap_or(0),
            u8::from_str_radix(&clean[2..4], 16).unwrap_or(0),
            u8::from_str_radix(&clean[4..6], 16).unwrap_or(0),
        ),
        3 => (
            u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0),
            u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0),
            u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0),
        ),
        _ => return Err(VectorCoreError::InvalidGeometry("Invalid HEX color code".to_string())),
    };
    Ok((r_u8 as f64 / 255.0, g_u8 as f64 / 255.0, b_u8 as f64 / 255.0))
}

fn format_hex_color(r: f64, g: f64, b: f64) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) / 2.0;

    if delta.abs() < 1e-6 {
        return (0.0, 0.0, l);
    }

    let s = if l > 0.5 { delta / (2.0 - max - min) } else { delta / (max + min) };

    let mut h = if max == r {
        (g - b) / delta + (if g < b { 6.0 } else { 0.0 })
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    h /= 6.0;

    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s.abs() < 1e-6 {
        return (l, l, l);
    }

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_luminance_adjustment() {
        // Pure Gray #808080 (Lightness ~0.5)
        let darkened = adjust_hex_luminance("#808080", 0.5).unwrap();
        // Lightness 0.5 * 0.5 = 0.25 -> #404040
        assert_eq!(darkened, "#404040");

        let brightened = adjust_hex_luminance("#404040", 2.0).unwrap();
        assert_eq!(brightened, "#808080");
    }
}
