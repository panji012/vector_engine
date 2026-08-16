use vector_core::{PathCommand, Point2D, VectorDocument, VectorPath};

pub struct BackgroundProcessor;

impl BackgroundProcessor {
    /// Detects and normalizes the background of an SVG document.
    /// If a background path is detected (covers >= 90% of the canvas area), it is completely replaced
    /// by a pristine rectangle covering exactly the canvas bounds, retaining the original color.
    pub fn normalize_background(doc: &mut VectorDocument) -> bool {
        if doc.paths.is_empty() {
            return false;
        }

        // 1. Determine target canvas bounds
        let (canvas_x, canvas_y, canvas_w, canvas_h) = if let Some(vb) = doc.view_box {
            vb
        } else if let (Some(w), Some(h)) = (doc.width, doc.height) {
            (0.0, 0.0, w, h)
        } else {
            // Fallback: Geometric union
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in &doc.paths {
                let poly = p.to_polygon();
                let (px1, py1, px2, py2) = poly.bounding_box();
                if px1 < min_x { min_x = px1; }
                if py1 < min_y { min_y = py1; }
                if px2 > max_x { max_x = px2; }
                if py2 > max_y { max_y = py2; }
            }
            if min_x == f64::INFINITY {
                return false; // No geometry
            }
            (min_x, min_y, max_x - min_x, max_y - min_y)
        };

        let canvas_area = canvas_w * canvas_h;
        if canvas_area <= 0.0 {
            return false;
        }

        // 2. Check if the first path actually covers >= 80% of the canvas area.
        // Only normalize background if path 0 is a simple 4-point rectangle (commands.len() <= 5)
        if doc.paths[0].commands.len() > 5 {
            return false;
        }

        let first_poly = doc.paths[0].to_polygon();
        let first_area = first_poly.area();
        if first_area < canvas_area * 0.80 {
            return false;
        }

        let bg_group_id = doc.paths[0].group_id;
        let bg_color = doc.paths[0].fill.clone();
        
        // Delete paths belonging ONLY to the background group if it's an isolated bg group, or just the background path itself
        let mut removed = false;
        doc.paths.retain(|p| {
            if !removed && p.group_id == bg_group_id {
                let p_area = p.to_polygon().area();
                if p_area >= canvas_area * 0.80 {
                    removed = true;
                    return false;
                }
            }
            true
        });

        if !removed {
            return false;
        }

        // 3. Create a clean rectangle
        let mut clean_bg = VectorPath::new(Some("normalized_background".to_string()));
        clean_bg.fill = bg_color;
        // Assign a completely unique group ID so ColorCombiner never merges it with foreground shapes!
        // This prevents foreground elements of the same color from punching holes in the background.
        clean_bg.group_id = usize::MAX;
        clean_bg.add_command(PathCommand::MoveTo(Point2D::new(canvas_x, canvas_y)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x + canvas_w, canvas_y)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x + canvas_w, canvas_y + canvas_h)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x, canvas_y + canvas_h)));
        clean_bg.add_command(PathCommand::Close);

        // 4. Insert at index 0 (bottom layer)
        doc.paths.insert(0, clean_bg);

        true
    }
}
