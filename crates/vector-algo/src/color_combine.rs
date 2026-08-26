use serde::{Deserialize, Serialize};
use vector_core::{VectorCoreError, VectorDocument, VectorPath};

/// Detailed statistics report for the Color Grouping operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorCombineReport {
    pub original_path_count: usize,
    pub number_of_color_groups: usize,
    pub combined_path_count: usize,
    pub group_details: Vec<ColorGroupDetail>,
}

/// Statistics for an individual color group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorGroupDetail {
    pub fill_color: String,
    pub input_paths: usize,
    pub output_paths: usize,
}

pub struct ColorCombiner;

impl ColorCombiner {
    /// Groups SVG document paths by fill color and combines consecutive sub-paths of each color group.
    pub fn combine_document(
        doc: &VectorDocument,
    ) -> Result<(VectorDocument, ColorCombineReport), VectorCoreError> {
        Self::combine_document_badge_aware(doc, false)
    }

    /// Badge-aware color grouping: if `enable_badge_grouping` is true and >= 2 spatial badge clusters are detected,
    /// groups SVG paths per badge cluster (`badge_id`), and merges color layers within each badge group.
    /// Otherwise falls back to global single-document color grouping.
    pub fn combine_document_badge_aware(
        doc: &VectorDocument,
        enable_badge_grouping: bool,
    ) -> Result<(VectorDocument, ColorCombineReport), VectorCoreError> {
        let original_path_count = doc.paths.len();

        if original_path_count == 0 {
            return Ok((
                doc.clone(),
                ColorCombineReport {
                    original_path_count: 0,
                    number_of_color_groups: 0,
                    combined_path_count: 0,
                    group_details: Vec::new(),
                },
            ));
        }

        if enable_badge_grouping {
            let clusters = crate::badge_cluster::BadgeDetector::detect_badges(doc, 12.0);
            if !clusters.is_empty() {
                let mut combined_doc = VectorDocument::new();
                combined_doc.width = doc.width;
                combined_doc.height = doc.height;
                combined_doc.view_box = doc.view_box;

                let mut group_details = Vec::new();
                let mut num_groups = 0;

                // 1. Detect and preserve global canvas background (covering >= 70% canvas)
                let (canvas_w, canvas_h) = if let Some(vb) = doc.view_box {
                    ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs())
                } else if let (Some(w), Some(h)) = (doc.width, doc.height) {
                    (w.abs(), h.abs())
                } else {
                    (1000.0, 1000.0)
                };

                let mut canvas_bg: Option<VectorPath> = None;
                if !doc.paths.is_empty() {
                    let p0_bbox = doc.paths[0].bounding_box();
                    let p0_w = (p0_bbox.2 - p0_bbox.0).abs();
                    let p0_h = (p0_bbox.3 - p0_bbox.1).abs();
                    if canvas_w > 0.0 && canvas_h > 0.0 && p0_w >= 0.70 * canvas_w && p0_h >= 0.70 * canvas_h {
                        let mut bg = doc.paths[0].clone();
                        bg.badge_id = None;
                        bg.group_id = usize::MAX;
                        canvas_bg = Some(bg);
                    }
                }

                if let Some(ref bg) = canvas_bg {
                    combined_doc.paths.push(bg.clone());
                }

                // 2. Process each badge cluster independently in exact Z-order sequence
                for cluster in &clusters {
                    let mut color_order: Vec<String> = Vec::new();
                    let mut color_map: std::collections::HashMap<String, VectorPath> = std::collections::HashMap::new();
                    let mut input_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

                    for &idx in &cluster.path_indices {
                        if idx == 0 && canvas_bg.is_some() {
                            continue;
                        }
                        let path = &doc.paths[idx];
                        let color_key = format!(
                            "fill:{}_stroke:{}_sw:{:?}_fr:{:?}",
                            path.fill.as_deref().unwrap_or("none").trim(),
                            path.stroke.as_deref().unwrap_or("none").trim(),
                            path.stroke_width,
                            path.fill_rule
                        );

                        *input_counts.entry(color_key.clone()).or_insert(0) += 1;

                        if let Some(compound) = color_map.get_mut(&color_key) {
                            for cmd in &path.commands {
                                compound.add_command(cmd.clone());
                            }
                        } else {
                            color_order.push(color_key.clone());
                            let mut new_path = VectorPath::new(None);
                            new_path.fill = path.fill.clone();
                            new_path.stroke = path.stroke.clone();
                            new_path.stroke_width = path.stroke_width;
                            new_path.fill_rule = path.fill_rule.clone();
                            new_path.group_id = path.group_id;
                            new_path.badge_id = Some(cluster.badge_id);
                            for cmd in &path.commands {
                                new_path.add_command(cmd.clone());
                            }
                            color_map.insert(color_key, new_path);
                        }
                    }

                    for (layer_idx, color_key) in color_order.iter().enumerate() {
                        if let Some(mut path) = color_map.remove(color_key) {
                            path.id = Some(format!("badge_{}_layer_{}", cluster.badge_id, layer_idx + 1));
                            combined_doc.paths.push(path.clone());
                            group_details.push(ColorGroupDetail {
                                fill_color: path.fill.clone().unwrap_or_else(|| "none".to_string()),
                                input_paths: *input_counts.get(color_key).unwrap_or(&0),
                                output_paths: 1,
                            });
                            num_groups += 1;
                        }
                    }
                }

                let combined_path_count = combined_doc.paths.len();
                let number_of_color_groups = num_groups;

                let report = ColorCombineReport {
                    original_path_count,
                    number_of_color_groups,
                    combined_path_count,
                    group_details,
                };

                return Ok((combined_doc, report));
            }
        }

        let mut combined_doc = VectorDocument::new();
        combined_doc.width = doc.width;
        combined_doc.height = doc.height;
        combined_doc.view_box = doc.view_box;

        let mut color_order: Vec<String> = Vec::new();
        let mut color_map: std::collections::HashMap<String, VectorPath> = std::collections::HashMap::new();
        let mut input_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut num_groups = 0;
        let mut group_details = Vec::new();

        for path in &doc.paths {
            let color_key = format!(
                "fill:{}_stroke:{}_sw:{:?}_fr:{:?}",
                path.fill.as_deref().unwrap_or("none").trim(),
                path.stroke.as_deref().unwrap_or("none").trim(),
                path.stroke_width,
                path.fill_rule
            );

            *input_counts.entry(color_key.clone()).or_insert(0) += 1;

            if let Some(compound) = color_map.get_mut(&color_key) {
                for cmd in &path.commands {
                    compound.add_command(cmd.clone());
                }
            } else {
                color_order.push(color_key.clone());
                let mut new_path = VectorPath::new(None);
                new_path.fill = path.fill.clone();
                new_path.stroke = path.stroke.clone();
                new_path.stroke_width = path.stroke_width;
                new_path.fill_rule = path.fill_rule.clone();
                new_path.group_id = path.group_id;
                for cmd in &path.commands {
                    new_path.add_command(cmd.clone());
                }
                color_map.insert(color_key, new_path);
            }
        }

        for (layer_idx, color_key) in color_order.iter().enumerate() {
            if let Some(mut path) = color_map.remove(color_key) {
                path.id = Some(format!("layer_{}", layer_idx + 1));
                combined_doc.paths.push(path.clone());
                group_details.push(ColorGroupDetail {
                    fill_color: path.fill.clone().unwrap_or_else(|| "none".to_string()),
                    input_paths: *input_counts.get(color_key).unwrap_or(&0),
                    output_paths: 1,
                });
                num_groups += 1;
            }
        }

        let combined_path_count = combined_doc.paths.len();
        let number_of_color_groups = num_groups;

        let report = ColorCombineReport {
            original_path_count,
            number_of_color_groups,
            combined_path_count,
            group_details,
        };

        Ok((combined_doc, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D};

    #[test]
    fn test_compound_path_color_grouping() {
        let mut doc = VectorDocument::new();

        // Color Group 1: Red (#FF0000) - 2 Separate Squares
        let mut r1 = VectorPath::new(Some("r1".to_string()));
        r1.fill = Some("#FF0000".to_string());
        r1.group_id = 1;
        r1.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(50.0, 0.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(50.0, 50.0)));
        r1.add_command(PathCommand::LineTo(Point2D::new(0.0, 50.0)));
        r1.add_command(PathCommand::Close);

        let mut r2 = VectorPath::new(Some("r2".to_string()));
        r2.fill = Some("#FF0000".to_string());
        r2.group_id = 1;
        r2.add_command(PathCommand::MoveTo(Point2D::new(25.0, 0.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(75.0, 0.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(75.0, 50.0)));
        r2.add_command(PathCommand::LineTo(Point2D::new(25.0, 50.0)));
        r2.add_command(PathCommand::Close);

        // Color Group 2: Blue (#0000FF) - 1 Square
        let mut b1 = VectorPath::new(Some("b1".to_string()));
        b1.fill = Some("#0000FF".to_string());
        b1.group_id = 2;
        b1.add_command(PathCommand::MoveTo(Point2D::new(100.0, 0.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(150.0, 0.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(150.0, 50.0)));
        b1.add_command(PathCommand::LineTo(Point2D::new(100.0, 50.0)));
        b1.add_command(PathCommand::Close);

        doc.paths.push(r1);
        doc.paths.push(r2);
        doc.paths.push(b1);

        let (combined_doc, report) = ColorCombiner::combine_document(&doc).unwrap();

        assert_eq!(report.original_path_count, 3);
        assert_eq!(report.number_of_color_groups, 2);
        assert_eq!(report.combined_path_count, 2);
        assert_eq!(combined_doc.paths.len(), 2);
    }
}
