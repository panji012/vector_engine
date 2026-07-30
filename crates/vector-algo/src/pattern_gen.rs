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
        Self::generate_pattern_styled(doc, grid_x, grid_y, "alternating", true)
    }

    /// Generates a seamless tiled pattern SVG with configurable layout style ("grid" or "alternating").
    pub fn generate_pattern_styled(
        doc: &VectorDocument,
        grid_x: u32,
        grid_y: u32,
        style: &str,
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

        let mut tiled_doc = VectorDocument::new();
        tiled_doc.width = Some(base_w * grid_x as f64);
        tiled_doc.height = Some(base_h * grid_y as f64);
        tiled_doc.view_box = Some((0.0, 0.0, base_w * grid_x as f64, base_h * grid_y as f64));

        let is_alternating = style.eq_ignore_ascii_case("alternating") || style.eq_ignore_ascii_case("flipped") || style.eq_ignore_ascii_case("bolak_balik");

        for r in 0..grid_y {
            for c in 0..grid_x {
                let dx = c as f64 * base_w;
                let dy = r as f64 * base_h;
                let flip_tile = is_alternating && ((r + c) % 2 == 1);

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
                            // Flipped / mirrored relative to tile cell bounds (bolak-balik)
                            Point2D::new(dx + (base_w - p.x), dy + (base_h - p.y))
                        } else {
                            // Standard parallel alignment (sejajar simetris)
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
                    tiled_doc.paths.push(translated_path);
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
