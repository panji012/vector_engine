use serde::{Deserialize, Serialize};
use vector_core::{PathCommand, VectorCoreError, VectorDocument};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternSuitabilityReport {
    pub is_suitable: bool,
    pub is_circular_shape: bool,
    pub is_square_shape: bool,
    pub detected_shape: String,
    pub suitability_score: f64,
    pub aspect_ratio_score: f64,
    pub closed_path_score: f64,
    pub edge_alignment_score: f64,
    pub coverage_score: f64,
    pub recommendations: Vec<String>,
}

pub struct PatternDetector;

impl PatternDetector {
    /// Analyzes an SVG `VectorDocument` for shape (circular / square) and pattern suitability.
    pub fn analyze_document(doc: &VectorDocument) -> Result<PatternSuitabilityReport, VectorCoreError> {
        let (width, height) = match doc.view_box {
            Some(vb) => (vb.2 - vb.0, vb.3 - vb.1),
            None => (doc.width.unwrap_or(100.0), doc.height.unwrap_or(100.0)),
        };

        if width <= 0.0 || height <= 0.0 {
            return Err(VectorCoreError::InvalidGeometry("Invalid document canvas dimensions".to_string()));
        }

        let mut recommendations = Vec::new();

        // 1. Aspect Ratio Score (Square 1:1 is ideal for circular / repeating tiles)
        let min_dim = width.min(height);
        let max_dim = width.max(height);
        let aspect_ratio = min_dim / max_dim;
        let aspect_ratio_score = aspect_ratio * 100.0;

        // 2. Shape Detection (Circular / Square)
        let is_aspect_square = aspect_ratio >= 0.75;

        let total_paths = doc.paths.len();
        let closed_paths = doc.paths.iter().filter(|p| {
            p.commands.iter().any(|cmd| matches!(cmd, PathCommand::Close))
        }).count();

        let closed_path_score = if total_paths == 0 {
            0.0
        } else {
            (closed_paths as f64 / total_paths as f64) * 100.0
        };

        let is_circular_shape = is_aspect_square && closed_path_score >= 60.0;
        let is_square_shape = is_aspect_square && closed_path_score >= 40.0;

        let detected_shape = if is_circular_shape {
            "circular".to_string()
        } else if is_square_shape {
            "square".to_string()
        } else {
            "other".to_string()
        };

        if is_circular_shape {
            recommendations.push("Desain terdeteksi berbentuk melingkar/bulat. Sangat cocok untuk pattern.".to_string());
        } else if is_square_shape {
            recommendations.push("Desain terdeteksi berbentuk persegi. Cocok untuk pattern.".to_string());
        } else {
            recommendations.push(format!("Canvas aspect ratio is {:.2}:1.", max_dim / min_dim));
        }

        // 3. Edge Alignment & Coverage Scores
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

        let canvas_area = width * height;
        let coverage_ratio = (total_area / canvas_area).clamp(0.0, 1.0);
        let coverage_score = if coverage_ratio >= 0.2 && coverage_ratio <= 0.85 {
            100.0
        } else {
            (1.0 - (coverage_ratio - 0.5).abs() * 2.0).clamp(0.2, 1.0) * 100.0
        };

        let suitability_score = (aspect_ratio_score * 0.35)
            + (closed_path_score * 0.35)
            + (edge_alignment_score * 0.15)
            + (coverage_score * 0.15);

        let has_badge_paths = doc.paths.iter().any(|p| p.badge_id.is_some());
        let badge_clusters = if !has_badge_paths {
            crate::badge_cluster::BadgeDetector::detect_badges(doc, 12.0)
        } else {
            Vec::new()
        };
        let is_badge = has_badge_paths || !badge_clusters.is_empty();

        let is_suitable = if is_badge {
            recommendations.push("Artwork terdeteksi sebagai Badge. Pattern generation dilewati.".to_string());
            false
        } else {
            suitability_score >= 50.0 || is_circular_shape || is_square_shape
        };

        Ok(PatternSuitabilityReport {
            is_suitable,
            is_circular_shape,
            is_square_shape,
            detected_shape,
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
        assert!(report.is_circular_shape);
        assert!(report.suitability_score >= 70.0);
        assert_eq!(report.aspect_ratio_score, 100.0);
        assert_eq!(report.closed_path_score, 100.0);
    }
}

