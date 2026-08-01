use vector_core::{VectorCoreError, VectorDocument};

pub struct LuminanceEngine;

impl LuminanceEngine {
    /// Computes adjusted luminance using the user formula:
    /// `target_base + (1.0 - target_base) * (control_pct / 100.0)`
    ///
    /// Examples:
    /// - Design 1: Target 60% (0.60), Control 50% => 0.60 + (1.0 - 0.60) * 0.50 = 0.80 (80%)
    /// - Design 2: Target 95% (0.95), Control 50% => 0.95 + (1.0 - 0.95) * 0.50 = 0.975 (97.5%)
    pub fn calculate_adjusted_luminance(target_base: f64, control_pct: f64) -> f64 {
        let remaining = (1.0 - target_base).max(0.0);
        let boost = remaining * (control_pct / 100.0);
        (target_base + boost).clamp(0.0, 1.0)
    }

    /// Adjusts the luminance / brightness of colors in a `VectorDocument` using a configurable multiplier `factor`.
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

    /// Applies Design 1 Variant:
    /// Target Luminance 60% with 50% control => 80% adjusted luminance for background.
    /// Foreground paths recolored in Blue palette scheme.
    pub fn apply_design1_variant(doc: &VectorDocument) -> Result<VectorDocument, VectorCoreError> {
        let target_bg_lum = Self::calculate_adjusted_luminance(0.60, 50.0); // 0.80 (80%)
        let blue_palette = vec![
            "#1A365D".to_string(), // Dark Navy
            "#2B6CB0".to_string(), // Royal Blue
            "#3182CE".to_string(), // Sky Blue
            "#4299E1".to_string(), // Light Blue
            "#63B3ED".to_string(), // Soft Blue
        ];
        Self::apply_theme_variant(doc, &blue_palette, target_bg_lum)
    }

    /// Applies Design 2 Variant:
    /// Target Luminance 95% with 50% control => 97.5% adjusted luminance for background.
    /// Foreground paths recolored in Brown palette scheme.
    pub fn apply_design2_variant(doc: &VectorDocument) -> Result<VectorDocument, VectorCoreError> {
        let target_bg_lum = Self::calculate_adjusted_luminance(0.95, 50.0); // 0.975 (97.5%)
        let brown_palette = vec![
            "#3D2314".to_string(), // Dark Coffee Brown
            "#5C3A21".to_string(), // Warm Chocolate
            "#8B5A2B".to_string(), // Golden Brown
            "#A0522D".to_string(), // Sienna
            "#CD853F".to_string(), // Peru Brown
        ];
        Self::apply_theme_variant(doc, &brown_palette, target_bg_lum)
    }

    /// Applies Dynamic Multi-Color Silhouette Variant:
    /// 1. Extracts all unique fill colors in the document and computes perceived relative luminance L for each color.
    /// 2. Sorts unique colors by luminance L from darkest to lightest.
    /// 3. Applies adaptive contrast thresholding with configurable `threshold` (0.0 to 1.0, default 0.50):
    ///    - Path 0 (bottom-most canvas background) or isolated group_id == usize::MAX: Always White `#FFFFFF`.
    ///    - If `line_art == true`: All remaining non-background paths map directly to Black `#000000`.
    ///    - Colors with relative luminance position P >= threshold: Mapped to White `#FFFFFF`.
    ///    - Colors with relative luminance position P < threshold: Mapped to Black `#000000`.
    /// Applies Dynamic Multi-Color Silhouette Variant with configurable options:
    /// - threshold: lightness threshold cutoff (0.0 to 1.0, defaults to 0.50 if <= 0)
    /// - line_art: if true, forces all non-background artwork paths to solid foreground color
    /// - bg_color_override: optional custom background color (HEX)
    /// - fg_color_override: optional custom foreground artwork color (HEX)
    /// - invert: if true, swaps background and foreground colors
    pub fn apply_silhouette_variant(doc: &VectorDocument, threshold: f64, line_art: bool) -> Result<VectorDocument, VectorCoreError> {
        Self::apply_silhouette_variant_full(doc, threshold, line_art, None, None, false, false)
    }

    pub fn apply_silhouette_variant_full(
        doc: &VectorDocument,
        threshold: f64,
        line_art: bool,
        bg_color_override: Option<&str>,
        fg_color_override: Option<&str>,
        invert: bool,
        bg_already_removed: bool,
    ) -> Result<VectorDocument, VectorCoreError> {
        let mut doc_out = doc.clone();
        if doc_out.paths.is_empty() {
            return Ok(doc_out);
        }

        let doc_w = doc_out.width.unwrap_or(1000.0);
        let doc_h = doc_out.height.unwrap_or(1000.0);

        let path0_bg_fill = if !bg_already_removed && !doc_out.paths.is_empty() {
            doc_out.paths[0].fill.clone()
        } else {
            None
        };

        // Resolve background & foreground colors
        let mut resolved_bg = bg_color_override
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or(path0_bg_fill)
            .unwrap_or_else(|| "#FFFFFF".to_string());

        let mut resolved_fg = fg_color_override
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("#000000")
            .to_string();

        if invert {
            std::mem::swap(&mut resolved_bg, &mut resolved_fg);
        }

        // Default cutoff threshold to 0.50 if non-positive or out of range
        let cutoff = if threshold <= 0.0 || threshold > 1.0 {
            0.50
        } else {
            threshold
        };

        let has_isolated_bg = if bg_already_removed {
            false
        } else {
            doc_out.paths.iter().any(|p| p.group_id == usize::MAX)
        };

        let path0_is_canvas_bg = if bg_already_removed || doc_out.paths.is_empty() {
            false
        } else {
            let (min_x, min_y, max_x, max_y) = doc_out.paths[0].bounding_box();
            let pw = (max_x - min_x).abs();
            let ph = (max_y - min_y).abs();
            pw >= doc_w * 0.85 && ph >= doc_h * 0.85
        };

        let bg_exists = has_isolated_bg || path0_is_canvas_bg;

        // 1. Extract unique fill colors & calculate luminance for each
        let mut color_lums: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for path in &doc_out.paths {
            if let Some(ref hex) = path.fill {
                if hex != "none" && !color_lums.contains_key(hex) {
                    if let Ok((r, g, b)) = parse_hex_color(hex) {
                        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                        color_lums.insert(hex.clone(), lum);
                    }
                }
            }
        }

        // 2. Sort unique colors by luminance ascending
        let mut sorted_colors: Vec<String> = color_lums.keys().cloned().collect();
        sorted_colors.sort_by(|a, b| {
            let lum_a = color_lums.get(a).cloned().unwrap_or(0.0);
            let lum_b = color_lums.get(b).cloned().unwrap_or(0.0);
            lum_a.partial_cmp(&lum_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 3. Build dynamic color mapping table
        let mut color_mapping: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let n = sorted_colors.len();

        if n == 1 {
            color_mapping.insert(sorted_colors[0].clone(), resolved_fg.clone());
        } else if n > 1 {
            let min_lum = color_lums.get(&sorted_colors[0]).cloned().unwrap_or(0.0);
            let max_lum = color_lums.get(&sorted_colors[n - 1]).cloned().unwrap_or(1.0);
            let lum_range = if max_lum > min_lum { max_lum - min_lum } else { 1.0 };

            color_mapping.insert(sorted_colors[n - 1].clone(), resolved_bg.clone());
            color_mapping.insert(sorted_colors[0].clone(), resolved_fg.clone());

            for color in sorted_colors.iter().take(n - 1).skip(1) {
                let lum = color_lums.get(color).cloned().unwrap_or(0.0);
                let rel_pos = (lum - min_lum) / lum_range;
                let normalized_lum = lum / 255.0;

                if line_art {
                    // In line art mode, dark detail/line art paths map to fg_color, light fill colors map to bg_color
                    if normalized_lum < 0.35 || rel_pos < cutoff {
                        color_mapping.insert(color.clone(), resolved_fg.clone());
                    } else {
                        color_mapping.insert(color.clone(), resolved_bg.clone());
                    }
                } else {
                    if normalized_lum < 0.15 || rel_pos < cutoff {
                        color_mapping.insert(color.clone(), resolved_fg.clone());
                    } else {
                        color_mapping.insert(color.clone(), resolved_bg.clone());
                    }
                }
            }
        }

        // 4. Apply mapping to paths (100% pure filled vector paths, stroke = None)
        for (idx, path) in doc_out.paths.iter_mut().enumerate() {
            let is_bg_layer = if has_isolated_bg {
                path.group_id == usize::MAX
            } else {
                idx == 0 && path0_is_canvas_bg
            };

            if is_bg_layer {
                path.fill = Some(resolved_bg.clone());
                path.stroke = None;
            } else if let Some(ref hex) = path.fill {
                let mapped = color_mapping.get(hex).cloned().unwrap_or_else(|| resolved_fg.clone());
                path.fill = Some(mapped);
                path.stroke = None;
            } else {
                path.fill = Some(resolved_fg.clone());
                path.stroke = None;
            }
        }

        // If background layer was removed or non-existent, inject background rectangle at index 0
        if !bg_exists {
            use vector_core::{PathCommand, Point2D, VectorPath};
            let bg_path = VectorPath {
                id: Some("canvas_bg".to_string()),
                commands: vec![
                    PathCommand::MoveTo(Point2D::new(0.0, 0.0)),
                    PathCommand::LineTo(Point2D::new(doc_w, 0.0)),
                    PathCommand::LineTo(Point2D::new(doc_w, doc_h)),
                    PathCommand::LineTo(Point2D::new(0.0, doc_h)),
                    PathCommand::Close,
                ],
                fill: Some(resolved_bg.clone()),
                stroke: None,
                stroke_width: None,
                fill_rule: None,
                group_id: usize::MAX,
            };
            doc_out.paths.insert(0, bg_path);
        }

        Ok(doc_out)
    }

    fn apply_theme_variant(
        doc: &VectorDocument,
        palette: &[String],
        bg_luminance: f64,
    ) -> Result<VectorDocument, VectorCoreError> {
        let mut doc_out = doc.clone();
        if doc_out.paths.is_empty() {
            return Ok(doc_out);
        }

        // Identify bottom-most path or group as background
        let is_bg_isolated = doc_out.paths.iter().any(|p| p.group_id == usize::MAX);

        let mut fg_index = 0;
        for path in &mut doc_out.paths {
            let is_bg = if is_bg_isolated {
                path.group_id == usize::MAX
            } else {
                false
            };


            if is_bg {
                // Background color: Inverted hue from primary fg color, lightness set to target bg_luminance
                let primary_fg = &palette[0];
                let (r, g, b) = parse_hex_color(primary_fg)?;
                let (h, s, _) = rgb_to_hsl(r, g, b);
                let inv_h = (h + 0.5) % 1.0; // Inverted hue
                let (bg_r, bg_g, bg_b) = hsl_to_rgb(inv_h, s.clamp(0.2, 0.6), bg_luminance);
                path.fill = Some(format_hex_color(bg_r, bg_g, bg_b));
            } else {
                // Foreground path recoloring from palette
                let color = &palette[fg_index % palette.len()];
                path.fill = Some(color.clone());
                fg_index += 1;
            }
        }

        Ok(doc_out)
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
    fn test_user_luminance_formulas() {
        // Design 1: Target 60%, Control 50% => 60% + 40% * 0.5 = 80% (0.80)
        let lum1 = LuminanceEngine::calculate_adjusted_luminance(0.60, 50.0);
        assert!((lum1 - 0.80).abs() < 1e-5);

        // Design 2: Target 95%, Control 50% => 95% + 5% * 0.5 = 97.5% (0.975)
        let lum2 = LuminanceEngine::calculate_adjusted_luminance(0.95, 50.0);
        assert!((lum2 - 0.975).abs() < 1e-5);
    }

    #[test]
    fn test_hex_luminance_adjustment() {
        let darkened = adjust_hex_luminance("#808080", 0.5).unwrap();
        assert_eq!(darkened, "#404040");

        let brightened = adjust_hex_luminance("#404040", 2.0).unwrap();
        assert_eq!(brightened, "#808080");
    }
}

