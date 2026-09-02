use vector_core::{PathCommand, Point2D, VectorDocument, VectorPath};

pub struct BackgroundProcessor;

impl BackgroundProcessor {
    /// Detects candidate background path index and fill color by analyzing path coverage relative to canvas size.
    pub fn find_background(doc: &VectorDocument, canvas_w: f64, canvas_h: f64) -> Option<(usize, String)> {
        let mut best_match: Option<(usize, String, f64)> = None;

        for (i, p) in doc.paths.iter().enumerate() {
            let fill = match p.fill {
                Some(ref f) if f != "none" && !f.is_empty() => f.clone(),
                _ => continue,
            };
            let bbox = p.bounding_box();
            let w = (bbox.2 - bbox.0).abs();
            let h = (bbox.3 - bbox.1).abs();
            let area = w * h;

            let pid = p.id.as_deref().unwrap_or("").to_lowercase();
            let is_explicit_bg = p.group_id == usize::MAX || pid.contains("background") || pid.contains("canvas_bg");

            if is_explicit_bg || (canvas_w > 0.0 && canvas_h > 0.0 && w >= 0.70 * canvas_w && h >= 0.70 * canvas_h) {
                if let Some((_, _, best_area)) = best_match {
                    if area > best_area || is_explicit_bg {
                        best_match = Some((i, fill, area));
                    }
                } else {
                    best_match = Some((i, fill, area));
                }
            }
        }
        best_match.map(|(i, color, _)| (i, color))
    }
    /// Detects and normalizes the background of an SVG document.
    /// Handles stencil compound paths for single-artwork documents while protecting badge grid layouts.
    pub fn normalize_background(doc: &mut VectorDocument) -> bool {
        if doc.paths.is_empty() {
            return false;
        }

        let original_path_count = doc.paths.len();

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
                return false;
            }
            (min_x, min_y, (max_x - min_x).max(100.0), (max_y - min_y).max(100.0))
        };

        let canvas_area = canvas_w * canvas_h;
        if canvas_area <= 0.0 {
            return false;
        }

        // Check if document is a true Badge Grid layout (originally contains >= 2 separate path elements with multiple spatial clusters)
        let is_badge_grid = if original_path_count >= 2 {
            let badge_clusters = crate::badge_cluster::BadgeDetector::detect_badges(doc, 12.0);
            badge_clusters.len() >= 2
        } else {
            false // Single-path document (like Design_027 stencil) is NEVER a badge grid!
        };

        // 2. For single-artwork documents (NOT badge grid), split compound stencil subpaths
        if !is_badge_grid {
            let mut exploded_paths = Vec::new();
            for path in &doc.paths {
                let subpaths = split_path_commands(&path.commands);
                if subpaths.len() <= 1 {
                    exploded_paths.push(path.clone());
                } else {
                    let sub0_bbox = get_commands_bbox(&subpaths[0]);
                    let sub0_w = (sub0_bbox.2 - sub0_bbox.0).abs();
                    let sub0_h = (sub0_bbox.3 - sub0_bbox.1).abs();
                    let is_compound_bg = canvas_w > 0.0 && canvas_h > 0.0
                        && sub0_w >= 0.50 * canvas_w
                        && sub0_h >= 0.50 * canvas_h;

                    if !is_compound_bg {
                        exploded_paths.push(path.clone());
                    } else {
                        let bg_color_hex = path.fill.clone().unwrap_or_else(|| "#FFFFFF".to_string());
                        let bg_is_light = is_color_light(&bg_color_hex);
                        let contrast_fg = if bg_is_light { "#000000".to_string() } else { "#FFFFFF".to_string() };

                        for (sub_idx, cmds) in subpaths.into_iter().enumerate() {
                            let mut sub_path = VectorPath::new(Some(format!("stencil_sub_{}", sub_idx + 1)));
                            sub_path.stroke = path.stroke.clone();
                            sub_path.stroke_width = path.stroke_width;
                            sub_path.fill_rule = path.fill_rule.clone();
                            sub_path.group_id = path.group_id;
                            sub_path.badge_id = path.badge_id;
                            sub_path.commands = cmds;

                            sub_path.fill = path.fill.clone();
                            exploded_paths.push(sub_path);
                        }
                    }
                }
            }
            doc.paths = exploded_paths;
        }

        // 3. Find candidate background path & color using accurate full-canvas bounding box scan
        let mut bg_indices = Vec::new();
        let (bg_idx, final_bg_color) = if let Some((idx, color)) = Self::find_background(doc, canvas_w, canvas_h) {
            bg_indices.push(idx);
            (Some(idx), color)
        } else {
            (None, "#FFFFFF".to_string())
        };

        // Remove detected background path(s) from document (in reverse order)
        bg_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in bg_indices {
            if idx < doc.paths.len() {
                doc.paths.remove(idx);
            }
        }

        // 4. Create a clean 100% full-canvas solid rectangle
        let mut clean_bg = VectorPath::new(Some("normalized_background".to_string()));
        clean_bg.fill = Some(final_bg_color);
        clean_bg.stroke = Some("none".to_string());
        clean_bg.stroke_width = Some(1.0);
        clean_bg.group_id = usize::MAX;
        clean_bg.add_command(PathCommand::MoveTo(Point2D::new(canvas_x, canvas_y)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x + canvas_w, canvas_y)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x + canvas_w, canvas_y + canvas_h)));
        clean_bg.add_command(PathCommand::LineTo(Point2D::new(canvas_x, canvas_y + canvas_h)));
        clean_bg.add_command(PathCommand::Close);

        // 5. Insert clean background at index 0 (bottom layer)
        doc.paths.insert(0, clean_bg);

        true
    }
}

fn split_path_commands(commands: &[PathCommand]) -> Vec<Vec<PathCommand>> {
    let mut subpaths = Vec::new();
    let mut current = Vec::new();
    for cmd in commands {
        if matches!(cmd, PathCommand::MoveTo(_)) && !current.is_empty() {
            subpaths.push(current);
            current = Vec::new();
        }
        current.push(cmd.clone());
    }
    if !current.is_empty() {
        subpaths.push(current);
    }
    subpaths
}

fn get_commands_bbox(cmds: &[PathCommand]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for cmd in cmds {
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
    if min_x.is_infinite() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

fn is_color_light(hex_color: &str) -> bool {
    let hex = hex_color.trim().trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            let luminance = 0.299 * (r as f64) + 0.587 * (g as f64) + 0.114 * (b as f64);
            return luminance > 128.0;
        }
    }
    true
}
