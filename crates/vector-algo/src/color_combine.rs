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

        let is_exploded_stencil = doc.paths.iter()
            .filter(|p| p.group_id != usize::MAX)
            .any(|p| p.id.as_deref().unwrap_or("").contains("_sub"));

        if enable_badge_grouping && !is_exploded_stencil {
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

                let bg_info = crate::background::BackgroundProcessor::find_background(doc, canvas_w, canvas_h);
                let bg_color_opt = bg_info.as_ref().map(|(_, color)| color.clone());

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

                // 2. Process each badge cluster independently ("per pulau") with 2-tier Z-order sorting
                // Combine paths of the SAME color within each badge cluster into 1 compound layer per color!
                for cluster in &clusters {
                    // Compute dominant palette colors for this badge cluster (colors covering >= 1.5% area)
                    let mut color_areas: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
                    let mut total_cluster_area = 0.0f64;

                    for &idx in &cluster.path_indices {
                        let p = &doc.paths[idx];
                        if let Some(ref fill) = p.fill {
                            if fill != "none" {
                                let bbox = p.bounding_box();
                                let area = (bbox.2 - bbox.0).abs() * (bbox.3 - bbox.1).abs();
                                *color_areas.entry(fill.to_lowercase()).or_insert(0.0) += area;
                                total_cluster_area += area;
                            }
                        }
                    }

                    let dominant_colors = build_cluster_dominant_palette(&color_areas);

                    let max_area = cluster
                        .path_indices
                        .iter()
                        .map(|&idx| {
                            let bbox = doc.paths[idx].bounding_box();
                            (bbox.2 - bbox.0).abs() * (bbox.3 - bbox.1).abs()
                        })
                        .fold(0.0f64, f64::max);

                    let mut sorted_indices = cluster.path_indices.clone();
                    sorted_indices.sort_by_key(|&idx| {
                        let bbox = doc.paths[idx].bounding_box();
                        let area = (bbox.2 - bbox.0).abs() * (bbox.3 - bbox.1).abs();
                        let is_container = max_area > 0.0 && (area / max_area) >= 0.05;
                        let tier = if is_container { 0 } else { 1 };
                        (tier, idx)
                    });

                    // Group sorted paths within this badge cluster by quantized dominant color key
                    let mut color_order: Vec<String> = Vec::new();
                    let mut color_map: std::collections::HashMap<String, VectorPath> =
                        std::collections::HashMap::new();

                    for &idx in &sorted_indices {
                        let path = &doc.paths[idx];
                        let bbox = path.bounding_box();
                        let area = (bbox.2 - bbox.0).abs() * (bbox.3 - bbox.1).abs();
                        let is_container = max_area > 0.0 && (area / max_area) >= 0.05;

                        let effective_fill = match path.fill.as_deref() {
                            Some(f) if !f.is_empty() && f != "none" => {
                                let f_lower = f.to_lowercase();
                                Some(quantize_to_dominant_palette(&f_lower, &dominant_colors))
                            }
                            _ => {
                                if path.stroke.is_none() {
                                    bg_color_opt.clone()
                                } else {
                                    path.fill.clone()
                                }
                            }
                        };

                        if path.stroke.is_none() {
                            if let (Some(ref fill_col), Some(ref bg_col)) = (&effective_fill, &bg_color_opt) {
                                if is_near_background(fill_col, bg_col) {
                                    // Check if this near-background shape is enclosed inside a solid non-background border/frame container in this badge cluster
                                    let is_enclosed = cluster.path_indices.iter().any(|&other_idx| {
                                        if other_idx == idx {
                                            return false;
                                        }
                                        let other_path = &doc.paths[other_idx];
                                        let obbox = other_path.bounding_box();
                                        let oarea = (obbox.2 - obbox.0).abs() * (obbox.3 - obbox.1).abs();
                                        let other_is_container = max_area > 0.0 && (oarea / max_area) >= 0.35;
                                        if !other_is_container {
                                            return false;
                                        }

                                        let other_fill = other_path.fill.as_deref().unwrap_or("none");
                                        let other_is_near_bg = is_near_background(other_fill, bg_col);
                                        if !other_is_near_bg || other_path.stroke.is_some() {
                                            let margin = 5.0;
                                            obbox.0 <= bbox.0 + margin
                                                && obbox.1 <= bbox.1 + margin
                                                && obbox.2 >= bbox.2 - margin
                                                && obbox.3 >= bbox.3 - margin
                                        } else {
                                            false
                                        }
                                    });

                                    if !is_enclosed {
                                        // Un-enclosed stain cloud around floating text -> DROP IT!
                                        continue;
                                    }
                                }
                            }
                        }

                        let color_key = format!(
                            "fill:{}_stroke:{}_sw:{:?}_fr:{:?}",
                            effective_fill.as_deref().unwrap_or("none").trim(),
                            path.stroke.as_deref().unwrap_or("none").trim(),
                            path.stroke_width,
                            path.fill_rule
                        );

                        if let Some(compound) = color_map.get_mut(&color_key) {
                            for cmd in &path.commands {
                                compound.add_command(cmd.clone());
                            }
                        } else {
                            color_order.push(color_key.clone());
                            let mut new_path = VectorPath::new(None);
                            new_path.fill = effective_fill;
                            new_path.stroke = path.stroke.clone();
                            new_path.stroke_width = path.stroke_width;
                            new_path.fill_rule = path.fill_rule.clone().or_else(|| Some("evenodd".to_string()));
                            new_path.group_id = path.group_id;
                            new_path.badge_id = Some(cluster.badge_id);
                            for cmd in &path.commands {
                                new_path.add_command(cmd.clone());
                            }
                            color_map.insert(color_key, new_path);
                        }
                    }

                    for (color_idx, color_key) in color_order.iter().enumerate() {
                        if let Some(mut combined_path) = color_map.remove(color_key) {
                            if combined_path.fill_rule.is_none() {
                                combined_path.fill_rule = Some("evenodd".to_string());
                            }
                            combined_path.id =
                                Some(format!("badge_{}_color_{}", cluster.badge_id, color_idx + 1));
                            combined_doc.paths.push(combined_path);
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

        let (canvas_w, canvas_h) = if let Some(vb) = doc.view_box {
            ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs())
        } else if let (Some(w), Some(h)) = (doc.width, doc.height) {
            (w, h)
        } else {
            (1000.0, 1000.0)
        };

        let bg_info = crate::background::BackgroundProcessor::find_background(doc, canvas_w, canvas_h);
        let bg_idx_opt = bg_info.as_ref().map(|(idx, _)| *idx);
        let bg_color_opt = bg_info.as_ref().map(|(_, color)| color.clone());

        for (idx, path) in doc.paths.iter().enumerate() {
            let pid = path.id.as_deref().unwrap_or("");
            let is_full_bg = path.group_id == usize::MAX
                || pid.contains("background")
                || pid.contains("canvas_bg")
                || Some(idx) == bg_idx_opt;

            if pid.starts_with("stencil_sub_") || is_full_bg {
                let mut standalone_path = path.clone();
                if is_full_bg {
                    standalone_path.group_id = usize::MAX;
                }
                combined_doc.paths.push(standalone_path);
                continue;
            }

            let effective_fill = match path.fill.as_deref() {
                Some(f) if !f.is_empty() && f != "none" => Some(f.to_string()),
                _ => {
                    if path.stroke.is_none() {
                        bg_color_opt.clone()
                    } else {
                        path.fill.clone()
                    }
                }
            };

            let color_key = format!(
                "fill:{}_stroke:{}_sw:{:?}_fr:{:?}",
                effective_fill.as_deref().unwrap_or("none").trim(),
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
                new_path.fill = effective_fill;
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

fn hex_to_rgb(hex_str: &str) -> (f64, f64, f64) {
    let clean = hex_str.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0) as f64 / 255.0;
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0) as f64 / 255.0;
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0) as f64 / 255.0;
        (r, g, b)
    } else if clean.len() == 3 {
        let r = u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0) as f64 / 255.0;
        let g = u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0) as f64 / 255.0;
        let b = u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0) as f64 / 255.0;
        (r, g, b)
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn hex_to_hsl(hex_str: &str) -> (f64, f64, f64) {
    let (r, g, b) = hex_to_rgb(hex_str);
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-6 {
        (0.0, 0.0, l)
    } else {
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let mut h = if (max - r).abs() < 1e-6 {
            (g - b) / d + (if g < b { 6.0 } else { 0.0 })
        } else if (max - g).abs() < 1e-6 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
        (h * 360.0, s, l)
    }
}

fn build_cluster_dominant_palette(
    color_areas: &std::collections::HashMap<String, f64>,
) -> Vec<String> {
    if color_areas.is_empty() {
        return Vec::new();
    }

    let mut sorted_colors: Vec<(String, f64)> = color_areas
        .iter()
        .map(|(c, &a)| (c.clone(), a))
        .collect();
    sorted_colors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut dominant_heads: Vec<String> = Vec::new();

    for (color, _area) in sorted_colors {
        let (h1, s1, l1) = hex_to_hsl(&color);

        let mut matches_existing = false;
        for head in &dominant_heads {
            let (h2, s2, l2) = hex_to_hsl(head);

            if l1 <= 0.15 && l2 <= 0.15 {
                matches_existing = true;
                break;
            }
            if l1 >= 0.88 && l2 >= 0.88 {
                matches_existing = true;
                break;
            }
            if s1 > 0.10 && s2 > 0.10 {
                let raw_h_diff = (h1 - h2).abs();
                let h_diff = raw_h_diff.min(360.0 - raw_h_diff);
                let l_diff = (l1 - l2).abs();
                if h_diff <= 15.0 && l_diff <= 0.20 {
                    matches_existing = true;
                    break;
                }
            }
        }

        if !matches_existing {
            dominant_heads.push(color);
        }
    }

    dominant_heads
}

fn quantize_to_dominant_palette(color: &str, dominant_colors: &[String]) -> String {
    if dominant_colors.is_empty() || dominant_colors.contains(&color.to_lowercase()) {
        return color.to_string();
    }

    let (h1, s1, l1) = hex_to_hsl(color);
    let mut best_match = color.to_string();
    let mut min_dist = f64::MAX;

    let is_dark1 = l1 <= 0.15;
    let is_light1 = l1 >= 0.88;

    for dom in dominant_colors {
        let (h2, s2, l2) = hex_to_hsl(dom);
        let is_dark2 = l2 <= 0.15;
        let is_light2 = l2 >= 0.88;

        if is_dark1 && !is_dark2 {
            continue;
        }
        if is_light1 && !is_light2 {
            continue;
        }
        if !is_dark1 && !is_light1 && (is_dark2 || is_light2) {
            continue;
        }

        let raw_h_diff = (h1 - h2).abs();
        let h_diff = raw_h_diff.min(360.0 - raw_h_diff);
        let l_diff = (l1 - l2).abs();

        if s1 > 0.15 && s2 > 0.15 && (h_diff > 15.0 || l_diff > 0.20) {
            continue;
        }

        let s_diff = (s1 - s2).abs();

        let dist = h_diff * 1.5 + l_diff * 30.0 + s_diff * 5.0;
        if dist < min_dist {
            min_dist = dist;
            best_match = dom.clone();
        }
    }

    best_match
}

fn is_near_background(color: &str, bg_color: &str) -> bool {
    let (h1, s1, l1) = hex_to_hsl(color);
    let (h2, s2, l2) = hex_to_hsl(bg_color);

    let raw_h_diff = (h1 - h2).abs();
    let h_diff = raw_h_diff.min(360.0 - raw_h_diff);
    let s_diff = (s1 - s2).abs();
    let l_diff = (l1 - l2).abs();

    h_diff <= 8.0 && s_diff <= 0.25 && l_diff <= 0.12
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
