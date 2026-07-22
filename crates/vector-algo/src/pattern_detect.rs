use serde::{Deserialize, Serialize};
use vector_core::{PathCommand, VectorCoreError, VectorDocument};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternSuitabilityReport {
    pub is_suitable: bool,
    pub suitability_score: f64,
    pub aspect_ratio_score: f64,
    pub closed_path_score: f64,
    pub edge_alignment_score: f64,
    pub coverage_score: f64,
    pub recommendations: Vec<String>,
}

pub struct PatternDetector;

impl PatternDetector {
    /// Analyzes an SVG `VectorDocument` for seamless pattern suitability and generates a score (0.0 to 100.0).
    pub fn analyze_document(doc: &VectorDocument) -> Result<PatternSuitabilityReport, VectorCoreError> {
        let (width, height) = match doc.view_box {
            Some(vb) => (vb.2 - vb.0, vb.3 - vb.1),
            None => (doc.width.unwrap_or(100.0), doc.height.unwrap_or(100.0)),
        };


        if width <= 0.0 || height <= 0.0 {
            return Err(VectorCoreError::InvalidGeometry("Invalid document canvas dimensions".to_string()));
        }

        let mut recommendations = Vec::new();

        // 1. Aspect Ratio Score (Square 1:1 is ideal for repeating tiles)
        let min_dim = width.min(height);
        let max_dim = width.max(height);
        let aspect_ratio = min_dim / max_dim;
        let aspect_ratio_score = aspect_ratio * 100.0;

        if aspect_ratio >= 0.95 {
            recommendations.push("Canvas has 1:1 square aspect ratio, ideal for seamless tiling.".to_string());
        } else {
            recommendations.push(format!("Canvas aspect ratio is {:.2}:1. Square 1:1 canvases tile more evenly.", max_dim / min_dim));
        }

        // 2. Closed Path Ratio Score
        let total_paths = doc.paths.len();
        let closed_paths = doc.paths.iter().filter(|p| {
            p.commands.iter().any(|cmd| matches!(cmd, PathCommand::Close))
        }).count();

        let closed_path_score = if total_paths == 0 {
            0.0
        } else {
            (closed_paths as f64 / total_paths as f64) * 100.0
        };

        if closed_path_score >= 75.0 {
            recommendations.push(format!("{:.0}% of paths are closed loops, producing clean tile boundaries.", closed_path_score));
        } else if total_paths > 0 {
            recommendations.push("Consider closing unsealed paths to prevent dangling tile edges.".to_string());
        }

        // 3. Edge Alignment & Continuity Score
        let mut edge_touch_count = 0;
        let mut total_area = 0.0;

        for path in &doc.paths {
            let mut p_min_x = f64::INFINITY;
            let mut p_min_y = f64::INFINITY;
            let mut p_max_x = f64::NEG_INFINITY;
            let mut p_max_y = f64::NEG_INFINITY;

            for cmd in &path.commands {
                let pt = match cmd {
                    PathCommand::MoveTo(p) | PathCommand::LineTo(p) => *p,
                    PathCommand::QuadTo { end, .. } => *end,
                    PathCommand::CubicTo { end, .. } => *end,
                    PathCommand::Close => continue,
                };
                if pt.x < p_min_x { p_min_x = pt.x; }
                if pt.y < p_min_y { p_min_y = pt.y; }
                if pt.x > p_max_x { p_max_x = pt.x; }
                if pt.y > p_max_y { p_max_y = pt.y; }
            }

            if p_min_x != f64::INFINITY {
                let pw = (p_max_x - p_min_x).max(0.0);
                let ph = (p_max_y - p_min_y).max(0.0);
                total_area += pw * ph;

                // Check proximity to left, right, top, bottom edges
                let margin = min_dim * 0.05;
                if p_min_x <= margin || p_max_x >= (width - margin) || p_min_y <= margin || p_max_y >= (height - margin) {
                    edge_touch_count += 1;
                }
            }
        }

        let edge_alignment_score = if total_paths == 0 {
            50.0
        } else {
            ((edge_touch_count as f64 / total_paths as f64) * 100.0).clamp(30.0, 100.0)
        };

        // 4. Canvas Coverage Score (30% - 80% coverage is optimal)
        let canvas_area = width * height;
        let coverage_ratio = (total_area / canvas_area).clamp(0.0, 1.0);
        let coverage_score = if coverage_ratio >= 0.2 && coverage_ratio <= 0.85 {
            100.0
        } else {
            (1.0 - (coverage_ratio - 0.5).abs() * 2.0).clamp(0.2, 1.0) * 100.0
        };

        // Weighted Overall Suitability Score
        let suitability_score = (aspect_ratio_score * 0.35)
            + (closed_path_score * 0.35)
            + (edge_alignment_score * 0.15)
            + (coverage_score * 0.15);

        let is_suitable = suitability_score >= 60.0;

        Ok(PatternSuitabilityReport {
            is_suitable,
            suitability_score,
            aspect_ratio_score,
            closed_path_score,
            edge_alignment_score,
            coverage_score,
            recommendations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D, VectorPath};

    #[test]
    fn test_pattern_detection_square_closed_svg() {
        let mut doc = VectorDocument::new();
        doc.view_box = Some((0.0, 0.0, 100.0, 100.0));


        let mut path = VectorPath::new(Some("square".to_string()));
        path.add_command(PathCommand::MoveTo(Point2D::new(10.0, 10.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(90.0, 10.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(90.0, 90.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(10.0, 90.0)));
        path.add_command(PathCommand::Close);

        doc.paths.push(path);

        let report = PatternDetector::analyze_document(&doc).unwrap();
        assert!(report.is_suitable);
        assert!(report.suitability_score >= 70.0);
        assert_eq!(report.aspect_ratio_score, 100.0);
        assert_eq!(report.closed_path_score, 100.0);
    }
}
