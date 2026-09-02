use vector_core::VectorDocument;

#[derive(Debug, Clone, PartialEq)]
pub struct BadgeCluster {
    pub badge_id: usize,
    pub path_indices: Vec<usize>,
    pub bounding_box: (f64, f64, f64, f64),
}

pub struct BadgeDetector;

impl BadgeDetector {
    /// Detects spatial badge clusters in a `VectorDocument`.
    /// Excludes canvas background paths (group_id == usize::MAX).
    /// Returns a list of `BadgeCluster` structs sorted top-to-bottom, left-to-right.
    pub fn detect_badges(doc: &VectorDocument, margin: f64) -> Vec<BadgeCluster> {
        let path_count = doc.paths.len();
        if path_count == 0 {
            return Vec::new();
        }

        // Determine total canvas dimensions
        let (canvas_w, canvas_h) = if let Some(vb) = doc.view_box {
            ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs())
        } else if let (Some(w), Some(h)) = (doc.width, doc.height) {
            (w.abs(), h.abs())
        } else {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in &doc.paths {
                let bbox = p.bounding_box();
                if bbox.0 < min_x { min_x = bbox.0; }
                if bbox.1 < min_y { min_y = bbox.1; }
                if bbox.2 > max_x { max_x = bbox.2; }
                if bbox.3 > max_y { max_y = bbox.3; }
            }
            if min_x.is_infinite() {
                (1000.0, 1000.0)
            } else {
                ((max_x - min_x).abs(), (max_y - min_y).abs())
            }
        };

        // Collect foreground path indices and their bounding boxes
        let mut fg_indices: Vec<usize> = Vec::new();
        let mut bboxes: Vec<(f64, f64, f64, f64)> = Vec::new();

        for (idx, path) in doc.paths.iter().enumerate() {
            if path.group_id == usize::MAX {
                continue; // Skip global canvas background
            }
            let bbox = path.bounding_box();
            let w = (bbox.2 - bbox.0).abs();
            let h = (bbox.3 - bbox.1).abs();

            // Ignore zero-area empty paths
            if w < 1e-5 && h < 1e-5 {
                continue;
            }

            // Ignore full-canvas background paths (covering >= 70% of canvas width AND height)
            if canvas_w > 0.0 && canvas_h > 0.0 && w >= 0.70 * canvas_w && h >= 0.70 * canvas_h {
                continue;
            }

            fg_indices.push(idx);
            bboxes.push(bbox);
        }

        if fg_indices.is_empty() {
            return Vec::new();
        }

        // Union-Find (Disjoint Set) data structure
        let n = fg_indices.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            let mut root = i;
            while root != parent[root] {
                root = parent[root];
            }
            let mut curr = i;
            while curr != root {
                let nxt = parent[curr];
                parent[curr] = root;
                curr = nxt;
            }
            root
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let root_i = find(parent, i);
            let root_j = find(parent, j);
            if root_i != root_j {
                parent[root_i] = root_j;
            }
        }

        // Compute adaptive proximity margin relative to canvas size (0.8% canvas size)
        let effective_margin = margin.max(0.008 * canvas_w).max(0.008 * canvas_h);
        let max_geom_dist = effective_margin + 6.0;
        let max_geom_dist_sq = max_geom_dist * max_geom_dist;

        // Collect subsampled points for fast geometry distance validation (max 20 points per path)
        let max_samples_per_path = 20;
        let sample_pts: Vec<Vec<vector_core::Point2D>> = doc.paths.iter().map(|p| {
            let mut all_pts = Vec::new();
            for cmd in &p.commands {
                match cmd {
                    vector_core::PathCommand::MoveTo(pt) | vector_core::PathCommand::LineTo(pt) => all_pts.push(*pt),
                    vector_core::PathCommand::QuadTo { control, end } => {
                        all_pts.push(vector_core::Point2D::new((control.x + end.x) * 0.5, (control.y + end.y) * 0.5));
                        all_pts.push(*end);
                    }
                    vector_core::PathCommand::CubicTo { control1, control2, end } => {
                        all_pts.push(vector_core::Point2D::new((control1.x + control2.x) * 0.5, (control1.y + control2.y) * 0.5));
                        all_pts.push(*end);
                    }
                    vector_core::PathCommand::Close => {}
                }
            }
            if all_pts.len() <= max_samples_per_path {
                all_pts
            } else {
                let stride = all_pts.len() / max_samples_per_path;
                all_pts.into_iter().step_by(stride.max(1)).take(max_samples_per_path).collect()
            }
        }).collect();

        // Connect paths whose bounding boxes overlap/are close AND actual geometry distance is within max_geom_dist
        for i in 0..n {
            let idx_i = fg_indices[i];
            let pts_i = &sample_pts[idx_i];
            if pts_i.is_empty() { continue; }

            for j in (i + 1)..n {
                let b1 = bboxes[i];
                let b2 = bboxes[j];
                if (b1.0 - effective_margin) <= b2.2
                    && (b1.2 + effective_margin) >= b2.0
                    && (b1.1 - effective_margin) <= b2.3
                    && (b1.3 + effective_margin) >= b2.1
                {
                    let idx_j = fg_indices[j];
                    let pts_j = &sample_pts[idx_j];
                    if pts_j.is_empty() { continue; }

                    // Fast 2D geometry distance validation with squared distance & early exit
                    let mut is_close = false;
                    for p1 in pts_i {
                        for p2 in pts_j {
                            let dx = p1.x - p2.x;
                            let dy = p1.y - p2.y;
                            if (dx * dx + dy * dy) <= max_geom_dist_sq {
                                is_close = true;
                                break;
                            }
                        }
                        if is_close {
                            break;
                        }
                    }

                    if is_close {
                        union(&mut parent, i, j);
                    }
                }
            }
        }



        // Group path indices by cluster root
        let mut clusters_map: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            clusters_map.entry(root).or_default().push(fg_indices[i]);
        }

        let canvas_area = canvas_w * canvas_h;
        let mut candidate_clusters: Vec<BadgeCluster> = Vec::new();

        let mut sorted_keys: Vec<usize> = clusters_map.keys().cloned().collect();
        sorted_keys.sort_unstable();
        for root in sorted_keys {
            let indices = clusters_map.remove(&root).unwrap();
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;

            for &idx in &indices {
                let bbox = doc.paths[idx].bounding_box();
                if bbox.0 < min_x { min_x = bbox.0; }
                if bbox.1 < min_y { min_y = bbox.1; }
                if bbox.2 > max_x { max_x = bbox.2; }
                if bbox.3 > max_y { max_y = bbox.3; }
            }

            let w = (max_x - min_x).abs();
            let h = (max_y - min_y).abs();
            let area = w * h;

            // Only consider clusters with area >= 0.2% of canvas area and dimension >= 2% canvas
            if canvas_area > 0.0 && area >= 0.002 * canvas_area && w >= 0.02 * canvas_w && h >= 0.02 * canvas_h {
                candidate_clusters.push(BadgeCluster {
                    badge_id: 0,
                    path_indices: indices,
                    bounding_box: (min_x, min_y, max_x, max_y),
                });
            }
        }

        if candidate_clusters.is_empty() {
            return Vec::new();
        }

        // Merge candidate sub-clusters whose bounding boxes touch or are close (belonging to 1 badge/sticker artwork)
        let max_gap_x = canvas_w * 0.05;
        let max_gap_y = canvas_h * 0.05;

        let mut merged_clusters: Vec<BadgeCluster> = candidate_clusters;
        let mut changed = true;
        while changed {
            changed = false;
            let mut next_merged: Vec<BadgeCluster> = Vec::new();

            for cluster in merged_clusters {
                let mut merged = false;
                for existing in &mut next_merged {
                    let gap_x = (cluster.bounding_box.0 - existing.bounding_box.2)
                        .max(existing.bounding_box.0 - cluster.bounding_box.2)
                        .max(0.0);
                    let gap_y = (cluster.bounding_box.1 - existing.bounding_box.3)
                        .max(existing.bounding_box.1 - cluster.bounding_box.3)
                        .max(0.0);

                    // Prevent merging two major badge clusters (both >= 10 paths)
                    // EXCEPT when their bounding boxes overlap significantly (>= 15% area ratio of the smaller cluster)
                    let intersection_w = (cluster.bounding_box.2.min(existing.bounding_box.2) - cluster.bounding_box.0.max(existing.bounding_box.0)).max(0.0);
                    let intersection_h = (cluster.bounding_box.3.min(existing.bounding_box.3) - cluster.bounding_box.1.max(existing.bounding_box.1)).max(0.0);
                    let intersection_area = intersection_w * intersection_h;

                    let area1 = (cluster.bounding_box.2 - cluster.bounding_box.0).abs() * (cluster.bounding_box.3 - cluster.bounding_box.1).abs();
                    let area2 = (existing.bounding_box.2 - existing.bounding_box.0).abs() * (existing.bounding_box.3 - existing.bounding_box.1).abs();
                    let min_area = area1.min(area2);

                    let significant_overlap = min_area > 0.0 && (intersection_area / min_area) >= 0.15;

                    let is_minor_merge = cluster.path_indices.len() < 10 || existing.path_indices.len() < 10 || significant_overlap;
                    if gap_x <= max_gap_x && gap_y <= max_gap_y && is_minor_merge {
                        existing.path_indices.extend(cluster.path_indices.clone());
                        existing.bounding_box.0 = existing.bounding_box.0.min(cluster.bounding_box.0);
                        existing.bounding_box.1 = existing.bounding_box.1.min(cluster.bounding_box.1);
                        existing.bounding_box.2 = existing.bounding_box.2.max(cluster.bounding_box.2);
                        existing.bounding_box.3 = existing.bounding_box.3.max(cluster.bounding_box.3);
                        merged = true;
                        changed = true;
                        break;
                    }
                }
                if !merged {
                    next_merged.push(cluster);
                }
            }
            merged_clusters = next_merged;
        }

        // Filter clusters using multi-factor BadgeConfidenceScore

        let mut valid_badge_clusters: Vec<BadgeCluster> = Vec::new();

        for cluster in &merged_clusters {
            let score = Self::calculate_confidence_score(doc, &cluster.path_indices, cluster.bounding_box, (canvas_w, canvas_h));
            let min_score = if cluster.path_indices.len() == 1 { 0.60 } else { 0.55 };
            if score >= min_score {
                valid_badge_clusters.push(cluster.clone());
            }
        }

        if valid_badge_clusters.is_empty() {
            return Vec::new();
        }

        let canvas_area = canvas_w * canvas_h;

        // Reject full-page illustrations if ANY cluster spans >= 50% of canvas area or >= 78% width & 70% height
        for cluster in &valid_badge_clusters {
            let bbox = cluster.bounding_box;
            let w = (bbox.2 - bbox.0).abs();
            let h = (bbox.3 - bbox.1).abs();
            let cluster_area = w * h;
            if canvas_area > 0.0 && (cluster_area >= 0.50 * canvas_area || (w >= 0.78 * canvas_w && h >= 0.70 * canvas_h)) {
                return Vec::new();
            }
        }

        // Require >= 2 candidate clusters to qualify as a multi-badge grid layout
        if valid_badge_clusters.len() < 2 {
            return Vec::new();
        }


        // Sort clusters top-to-bottom, left-to-right (Tolerating staggered organic placement up to 12% canvas height)
        valid_badge_clusters.sort_by(|a, b| {
            let y_diff = (a.bounding_box.1 - b.bounding_box.1).abs();
            let tolerance = 0.12 * canvas_h;
            if y_diff > tolerance {
                a.bounding_box.1.partial_cmp(&b.bounding_box.1).unwrap()
            } else {
                a.bounding_box.0.partial_cmp(&b.bounding_box.0).unwrap()
            }
        });

        // Assign 1-indexed badge IDs
        for (i, cluster) in valid_badge_clusters.iter_mut().enumerate() {
            cluster.badge_id = i + 1;
        }

        // Include any unassigned foreground path whose center lies within a badge bounding box
        let assigned_set: std::collections::HashSet<usize> = valid_badge_clusters
            .iter()
            .flat_map(|c| c.path_indices.iter().cloned())
            .collect();

        for &idx in &fg_indices {
            if assigned_set.contains(&idx) {
                continue;
            }
            let bbox = doc.paths[idx].bounding_box();
            let cx = (bbox.0 + bbox.2) * 0.5;
            let cy = (bbox.1 + bbox.3) * 0.5;

            // Find best matching containing badge
            let mut best_cluster_idx: Option<usize> = None;
            let mut min_dist_to_center = f64::INFINITY;

            for (c_idx, cluster) in valid_badge_clusters.iter().enumerate() {
                let c_bbox = cluster.bounding_box;
                let pad_x = (c_bbox.2 - c_bbox.0).abs() * 0.05;
                let pad_y = (c_bbox.3 - c_bbox.1).abs() * 0.05;

                if cx >= (c_bbox.0 - pad_x)
                    && cx <= (c_bbox.2 + pad_x)
                    && cy >= (c_bbox.1 - pad_y)
                    && cy <= (c_bbox.3 + pad_y)
                {
                    let c_cx = (c_bbox.0 + c_bbox.2) * 0.5;
                    let c_cy = (c_bbox.1 + c_bbox.3) * 0.5;
                    let dist = (cx - c_cx).hypot(cy - c_cy);
                    if dist < min_dist_to_center {
                        min_dist_to_center = dist;
                        best_cluster_idx = Some(c_idx);
                    }
                }
            }

            if let Some(target_idx) = best_cluster_idx {
                valid_badge_clusters[target_idx].path_indices.push(idx);
            }
        }

        valid_badge_clusters
    }

    /// Multi-Factor Badge Confidence Scoring Engine (0.0 to 1.0)
    /// Evaluates: Aspect Ratio, Area-Weighted Symmetry, Concentric Layering, Margin, Outer Boundary, and Typography.
    pub fn calculate_confidence_score(
        doc: &VectorDocument,
        indices: &[usize],
        bbox: (f64, f64, f64, f64),
        canvas_dim: (f64, f64),
    ) -> f64 {
        let w = (bbox.2 - bbox.0).abs();
        let h = (bbox.3 - bbox.1).abs();
        if w <= 0.0 || h <= 0.0 {
            return 0.0;
        }

        // 1. Aspect Ratio & Compactness (20% weight)
        let aspect = w / h;
        let aspect_score = if aspect >= 0.75 && aspect <= 1.33 {
            1.0
        } else if aspect >= 0.5 && aspect <= 2.0 {
            0.6
        } else {
            0.2
        };

        // 2. Area-Weighted Bilateral Left-Right Symmetry (25% weight)
        let cx = (bbox.0 + bbox.2) / 2.0;
        let mut left_area = 0.0;
        let mut right_area = 0.0;
        let tol = 0.05 * w;

        for &idx in indices {
            let p_bbox = doc.paths[idx].bounding_box();
            let pw = (p_bbox.2 - p_bbox.0).abs();
            let ph = (p_bbox.3 - p_bbox.1).abs();
            let p_area = pw * ph;
            let p_cx = (p_bbox.0 + p_bbox.2) / 2.0;

            if (p_cx - cx).abs() <= tol {
                left_area += p_area * 0.5;
                right_area += p_area * 0.5;
            } else if p_cx < cx {
                left_area += p_area;
            } else {
                right_area += p_area;
            }
        }
        let total_area = (left_area + right_area).max(1e-5);
        let area_diff = (left_area - right_area).abs() / total_area;
        let symmetry_score = (1.0 - area_diff * 1.5).clamp(0.2, 1.0);

        // 3. Concentric Layering & Path Density (20% weight)
        let path_cnt = indices.len();
        let layering_score = if path_cnt >= 3 {
            1.0
        } else if path_cnt == 2 {
            0.7
        } else {
            0.2
        };

        // 4. Canvas Margin / Padding (15% weight)
        let (cw, ch) = canvas_dim;
        let margin_score = if cw > 0.0 && ch > 0.0 {
            let pad_x = bbox.0.min((cw - bbox.2).max(0.0));
            let pad_y = bbox.1.min((ch - bbox.3).max(0.0));
            if pad_x >= 0.01 * cw && pad_y >= 0.01 * ch {
                1.0
            } else {
                0.5
            }
        } else {
            0.8
        };

        // 5. Outer Boundary Contour wrapping inner paths (20% weight)
        let mut outer_score = 0.0;
        for &idx in indices {
            let p_bbox = doc.paths[idx].bounding_box();
            let pw = (p_bbox.2 - p_bbox.0).abs();
            let ph = (p_bbox.3 - p_bbox.1).abs();
            if pw >= 0.75 * w && ph >= 0.75 * h {
                outer_score = 1.0;
                break;
            }
        }

        // 6. Typography & Micro-Element Detection (Text, Ribbon, Stars)
        let mut has_text_or_ribbon = false;
        for &idx in indices {
            let p = &doc.paths[idx];
            if let Some(ref id) = p.id {
                let id_lower = id.to_lowercase();
                if id_lower.contains("text") || id_lower.contains("letter") || id_lower.contains("font") || id_lower.contains("ribbon") || id_lower.contains("banner") {
                    has_text_or_ribbon = true;
                    break;
                }
            }
            let p_bbox = p.bounding_box();
            let pw = (p_bbox.2 - p_bbox.0).abs();
            let ph = (p_bbox.3 - p_bbox.1).abs();
            if pw <= 0.35 * w && ph <= 0.35 * h && p.commands.len() >= 2 {
                has_text_or_ribbon = true;
            }
        }

        // MANDATORY RULE: A badge MUST have either an Outer Boundary Container OR Text/Ribbon elements.
        // If it has NEITHER, it is a decorative pattern motif/illustration, NOT a badge!
        if outer_score == 0.0 && !has_text_or_ribbon {
            return 0.0;
        }

        (aspect_score * 0.20)
            + (symmetry_score * 0.25)
            + (layering_score * 0.20)
            + (margin_score * 0.15)
            + (outer_score * 0.20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D, VectorPath};

    #[test]
    fn test_badge_detection_four_clusters() {
        let mut doc = VectorDocument::new();

        // Badge 1 (Top-Left): 20x20 at (10, 10)
        let mut b1_p1 = VectorPath::new(None);
        b1_p1.add_command(PathCommand::MoveTo(Point2D::new(10.0, 10.0)));
        b1_p1.add_command(PathCommand::LineTo(Point2D::new(30.0, 30.0)));
        doc.paths.push(b1_p1);

        // Badge 2 (Top-Right): 20x20 at (100, 10)
        let mut b2_p1 = VectorPath::new(None);
        b2_p1.add_command(PathCommand::MoveTo(Point2D::new(100.0, 10.0)));
        b2_p1.add_command(PathCommand::LineTo(Point2D::new(120.0, 30.0)));
        doc.paths.push(b2_p1);

        // Badge 3 (Bottom-Left): 20x20 at (10, 100)
        let mut b3_p1 = VectorPath::new(None);
        b3_p1.add_command(PathCommand::MoveTo(Point2D::new(10.0, 100.0)));
        b3_p1.add_command(PathCommand::LineTo(Point2D::new(30.0, 120.0)));
        doc.paths.push(b3_p1);

        // Badge 4 (Bottom-Right): 20x20 at (100, 100)
        let mut b4_p1 = VectorPath::new(None);
        b4_p1.add_command(PathCommand::MoveTo(Point2D::new(100.0, 100.0)));
        b4_p1.add_command(PathCommand::LineTo(Point2D::new(120.0, 120.0)));
        doc.paths.push(b4_p1);

        let clusters = BadgeDetector::detect_badges(&doc, 10.0);
        assert_eq!(clusters.len(), 4);
        assert_eq!(clusters[0].badge_id, 1);
        assert_eq!(clusters[1].badge_id, 2);
        assert_eq!(clusters[2].badge_id, 3);
        assert_eq!(clusters[3].badge_id, 4);
    }

    #[test]
    fn test_real_svg_inputs_badge_clustering() {
        use vector_io::SvgParser;

        // 1. Remix_Output_4 (4).svg is a single artwork illustration (non-badge) -> MUST detect 0 badge clusters
        if let Ok(content) = std::fs::read_to_string("../../input/Remix_Output_4 (4).svg") {
            if let Ok(doc) = SvgParser::parse_str(&content) {
                let clusters = BadgeDetector::detect_badges(&doc, 12.0);
                assert_eq!(clusters.len(), 0, "Single illustration artwork must detect 0 badge clusters!");
            }
        }

        // 2. AI_Remix_Output_2 (11).svg is a 4-badge grid layout -> MUST detect EXACTLY 4 badge clusters
        if let Ok(content) = std::fs::read_to_string("../../input/AI_Remix_Output_2 (11).svg") {
            if let Ok(doc) = SvgParser::parse_str(&content) {
                let clusters = BadgeDetector::detect_badges(&doc, 12.0);
                assert_eq!(clusters.len(), 4, "4-Badge Grid artwork must detect EXACTLY 4 badge clusters!");
                assert_eq!(clusters[0].badge_id, 1);
                assert_eq!(clusters[1].badge_id, 2);
                assert_eq!(clusters[2].badge_id, 3);
                assert_eq!(clusters[3].badge_id, 4);
            }
        }
    }
}


