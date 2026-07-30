use crate::pattern_detect::PatternDetector;
use vector_core::{PathCommand, Point2D, VectorCoreError, VectorDocument, VectorPath};

pub struct PatternGenerator;

impl PatternGenerator {
    /// Generates a seamless tiled pattern SVG `VectorDocument` ONLY if PatternDetector approves.
    pub fn generate_pattern(
        doc: &VectorDocument,
        grid_x: u32,
        grid_y: u32,
    ) -> Result<VectorDocument, VectorCoreError> {
        Self::generate_pattern_full_config(doc, grid_x, grid_y, "slanted_left", -0.5, true, true)
    }

    /// Generates a seamless tiled pattern SVG with configurable layout style ("grid", "alternating", or "slanted_left").
    pub fn generate_pattern_styled(
        doc: &VectorDocument,
        grid_x: u32,
        grid_y: u32,
        style: &str,
        detect_suitable_only: bool,
    ) -> Result<VectorDocument, VectorCoreError> {
        Self::generate_pattern_full_config(doc, grid_x, grid_y, style, -0.5, true, detect_suitable_only)
    }

    /// Full configurable seamless pattern generator supporting slanted offset, alternating tiles, and toroidal edge wraparound.
    pub fn generate_pattern_full_config(
        doc: &VectorDocument,
        grid_x: u32,
        grid_y: u32,
        style: &str,
        slant_offset_ratio: f64,
        wraparound_seamless: bool,
        detect_suitable_only: bool,
    ) -> Result<VectorDocument, VectorCoreError> {
        if grid_x == 0 || grid_y == 0 {
            return Err(VectorCoreError::InvalidGeometry("Grid dimensions must be > 0".to_string()));
        }

        // Gatekeeper Check: Optional screening
        if detect_suitable_only {
            let report = PatternDetector::analyze_document(doc)?;
            if !report.is_suitable {
                return Err(VectorCoreError::InvalidGeometry(format!(
                    "SVG is not suitable for seamless pattern generation. Suitability score is {:.1}/100. Recommendations: {}",
                    report.suitability_score,
                    report.recommendations.join("; ")
                )));
            }
        }

        let (base_w, base_h) = match doc.view_box {
            Some(vb) => (vb.2 - vb.0, vb.3 - vb.1),
            None => (doc.width.unwrap_or(100.0), doc.height.unwrap_or(100.0)),
        };

        let total_w = base_w * grid_x as f64;
        let total_h = base_h * grid_y as f64;

        let mut tiled_doc = VectorDocument::new();
        tiled_doc.width = Some(total_w);
        tiled_doc.height = Some(total_h);
        tiled_doc.view_box = Some((0.0, 0.0, total_w, total_h));

        let is_alternating = style.eq_ignore_ascii_case("alternating") || style.eq_ignore_ascii_case("flipped") || style.eq_ignore_ascii_case("bolak_balik");
        let is_slanted = style.contains("slanted") || style.contains("miring") || style.contains("half_drop");

        let mut candidate_paths: Vec<VectorPath> = Vec::new();

        // Extended grid coverage to account for slanted offsets
        let extra_cols = if is_slanted { 2 } else { 0 };
        let col_start = 0 - extra_cols;
        let col_end = grid_x as i32 + extra_cols;

        for r in 0..grid_y as i32 {
            let slant_shift = if is_slanted {
                r as f64 * (base_w * slant_offset_ratio)
            } else {
                0.0
            };

            for c in col_start..col_end {
                let dx = c as f64 * base_w + slant_shift;
                let dy = r as f64 * base_h;
                let flip_tile = is_alternating && ((r.abs() + c.abs()) % 2 == 1);

                for (_idx, path) in doc.paths.iter().enumerate() {
                    let mut translated_path = VectorPath::new(Some(format!(
                        "{}_tile_{}_{}",
                        path.id.as_deref().unwrap_or("path"),
                        c,
                        r
                    )));

                    translated_path.fill = path.fill.clone();
                    translated_path.stroke = path.stroke.clone();
                    translated_path.stroke_width = path.stroke_width;

                    let transform_pt = |p: &Point2D| -> Point2D {
                        if flip_tile {
                            Point2D::new(dx + (base_w - p.x), dy + (base_h - p.y))
                        } else {
                            Point2D::new(p.x + dx, p.y + dy)
                        }
                    };

                    for cmd in &path.commands {
                        let translated_cmd = match cmd {
                            PathCommand::MoveTo(p) => PathCommand::MoveTo(transform_pt(p)),
                            PathCommand::LineTo(p) => PathCommand::LineTo(transform_pt(p)),
                            PathCommand::QuadTo { control, end } => PathCommand::QuadTo {
                                control: transform_pt(control),
                                end: transform_pt(end),
                            },
                            PathCommand::CubicTo { control1, control2, end } => PathCommand::CubicTo {
                                control1: transform_pt(control1),
                                control2: transform_pt(control2),
                                end: transform_pt(end),
                            },
                            PathCommand::Close => PathCommand::Close,
                        };
                        translated_path.add_command(translated_cmd);
                    }
                    candidate_paths.push(translated_path);
                }
            }
        }

        // Toroidal Edge Wraparound: duplicate geometry extending across left/right/top/bottom boundaries
        for path in candidate_paths {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;

            for cmd in &path.commands {
                let pt = match cmd {
                    PathCommand::MoveTo(p) | PathCommand::LineTo(p) => *p,
                    PathCommand::QuadTo { end, .. } => *end,
                    PathCommand::CubicTo { end, .. } => *end,
                    PathCommand::Close => continue,
                };
                if pt.x < min_x { min_x = pt.x; }
                if pt.y < min_y { min_y = pt.y; }
                if pt.x > max_x { max_x = pt.x; }
                if pt.y > max_y { max_y = pt.y; }
            }

            if min_x == f64::INFINITY {
                continue;
            }

            // Primary path inclusion
            let is_visible = max_x >= 0.0 && min_x <= total_w && max_y >= 0.0 && min_y <= total_h;
            if is_visible {
                tiled_doc.paths.push(path.clone());
            }

            // Wraparound duplicates when wraparound_seamless is enabled
            if wraparound_seamless {
                let offsets_x = if min_x < 0.0 {
                    vec![total_w]
                } else if max_x > total_w {
                    vec![-total_w]
                } else {
                    vec![]
                };

                for shift_x in offsets_x {
                    let mut wrapped_path = VectorPath::new(Some(format!("{}_wrap_x", path.id.as_deref().unwrap_or("path"))));
                    wrapped_path.fill = path.fill.clone();
                    wrapped_path.stroke = path.stroke.clone();
                    wrapped_path.stroke_width = path.stroke_width;

                    for cmd in &path.commands {
                        let shift_cmd = match cmd {
                            PathCommand::MoveTo(p) => PathCommand::MoveTo(Point2D::new(p.x + shift_x, p.y)),
                            PathCommand::LineTo(p) => PathCommand::LineTo(Point2D::new(p.x + shift_x, p.y)),
                            PathCommand::QuadTo { control, end } => PathCommand::QuadTo {
                                control: Point2D::new(control.x + shift_x, control.y),
                                end: Point2D::new(end.x + shift_x, end.y),
                            },
                            PathCommand::CubicTo { control1, control2, end } => PathCommand::CubicTo {
                                control1: Point2D::new(control1.x + shift_x, control1.y),
                                control2: Point2D::new(control2.x + shift_x, control2.y),
                                end: Point2D::new(end.x + shift_x, end.y),
                            },
                            PathCommand::Close => PathCommand::Close,
                        };
                        wrapped_path.add_command(shift_cmd);
                    }
                    tiled_doc.paths.push(wrapped_path);
                }
            }
        }

        Ok(tiled_doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D, VectorPath};

    #[test]
    fn test_pattern_generator_approval_and_color_preservation() {
        let mut doc = VectorDocument::new();
        doc.view_box = Some((0.0, 0.0, 100.0, 100.0));

        let mut path = VectorPath::new(Some("star".to_string()));
        path.fill = Some("#FF5722".to_string()); // Original Deep Orange Fill
        path.stroke = Some("#212121".to_string()); // Original Dark Stroke
        path.stroke_width = Some(1.5);

        path.add_command(PathCommand::MoveTo(Point2D::new(10.0, 10.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(90.0, 10.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(90.0, 90.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(10.0, 90.0)));
        path.add_command(PathCommand::Close);

        doc.paths.push(path);

        // Test 3x3 grid generation
        let tiled_doc = PatternGenerator::generate_pattern(&doc, 3, 3).unwrap();

        // 3x3 grid of 1 path = 9 tiled paths
        assert_eq!(tiled_doc.paths.len(), 9);

        // Check color preservation on all tiles
        for tile in &tiled_doc.paths {
            assert_eq!(tile.fill.as_deref(), Some("#FF5722"));
            assert_eq!(tile.stroke.as_deref(), Some("#212121"));
            assert_eq!(tile.stroke_width, Some(1.5));
        }

        assert_eq!(tiled_doc.view_box, Some((0.0, 0.0, 300.0, 300.0)));
    }
}
