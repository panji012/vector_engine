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

        // Connect paths whose bounding boxes overlap or are within margin distance
        for i in 0..n {
            for j in (i + 1)..n {
                let b1 = bboxes[i];
                let b2 = bboxes[j];
                if (b1.0 - margin) <= b2.2
                    && (b1.2 + margin) >= b2.0
                    && (b1.1 - margin) <= b2.3
                    && (b1.3 + margin) >= b2.1
                {
                    union(&mut parent, i, j);
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

        for (_root, indices) in clusters_map {
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

            // Only consider major clusters with area >= 0.5% of canvas area and dimension >= 3% canvas
            if canvas_area > 0.0 && area >= 0.005 * canvas_area && w >= 0.03 * canvas_w && h >= 0.03 * canvas_h {
                candidate_clusters.push(BadgeCluster {
                    badge_id: 0,
                    path_indices: indices,
                    bounding_box: (min_x, min_y, max_x, max_y),
                });
            }
        }

        // Must have at least 2 major clusters to be considered a Multi-Badge Grid
        if candidate_clusters.len() < 2 {
            return Vec::new();
        }

        // Check area ratio among major clusters to distinguish true uniform badge grids from single posters
        let mut min_area = f64::INFINITY;
        let mut max_area = f64::NEG_INFINITY;

        for c in &candidate_clusters {
            let w = (c.bounding_box.2 - c.bounding_box.0).abs();
            let h = (c.bounding_box.3 - c.bounding_box.1).abs();
            let a = w * h;
            if a < min_area { min_area = a; }
            if a > max_area { max_area = a; }
        }

        let area_ratio = if min_area > 0.0 { max_area / min_area } else { 999.0 };

        // If area ratio is > 5.0, it is a single illustration poster with text/decorations, NOT a badge grid!
        if area_ratio > 5.0 {
            return Vec::new();
        }

        // Sort clusters top-to-bottom, left-to-right
        candidate_clusters.sort_by(|a, b| {
            let y_diff = (a.bounding_box.1 - b.bounding_box.1).abs();
            if y_diff > margin * 1.5 {
                a.bounding_box.1.partial_cmp(&b.bounding_box.1).unwrap()
            } else {
                a.bounding_box.0.partial_cmp(&b.bounding_box.0).unwrap()
            }
        });

        // Assign 1-indexed badge IDs
        for (i, cluster) in candidate_clusters.iter_mut().enumerate() {
            cluster.badge_id = i + 1;
        }

        candidate_clusters
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
}
