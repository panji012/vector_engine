use crate::pattern_detect::PatternDetector;
use vector_core::{PathCommand, Point2D, VectorCoreError, VectorDocument, VectorPath};
use vector_offset::flatten::CurveFlattener;

fn is_base_background_rect(cmds: &[PathCommand], base_w: f64, base_h: f64, is_bottom_path: bool) -> bool {
    if cmds.is_empty() {
        return false;
    }
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
        return false;
    }

    let rect_w = (max_x - min_x).max(0.0);
    let rect_h = (max_y - min_y).max(0.0);
    let rect_area = rect_w * rect_h;
    let tile_area = base_w * base_h;
    let coverage = if tile_area > 0.0 { rect_area / tile_area } else { 0.0 };

    let margin = 5.0;
    let touches_boundary = min_x <= margin && min_y <= margin && max_x >= (base_w - margin) && max_y >= (base_h - margin);

    touches_boundary || (is_bottom_path && coverage >= 0.90)
}

fn split_subpaths(commands: &[PathCommand]) -> Vec<Vec<PathCommand>> {
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

fn snap_point_to_rect_boundary(p: Point2D, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Point2D {
    let snap_tol = 0.05; // 0.05px sub-pixel edge snapping tolerance for zero gap seam fitting
    let mut x = p.x;
    let mut y = p.y;

    if (x - min_x).abs() <= snap_tol {
        x = min_x;
    } else if (x - max_x).abs() <= snap_tol {
        x = max_x;
    }

    if (y - min_y).abs() <= snap_tol {
        y = min_y;
    } else if (y - max_y).abs() <= snap_tol {
        y = max_y;
    }

    Point2D::new(x, y)
}

fn clip_polygon_to_rect(pts: &[Point2D], min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<Point2D> {
    if pts.len() < 3 {
        return Vec::new();
    }

    let clip_halfplane = |poly: Vec<Point2D>, inside: &dyn Fn(&Point2D) -> bool, intersect: &dyn Fn(&Point2D, &Point2D) -> Point2D| -> Vec<Point2D> {
        let mut out = Vec::new();
        if poly.is_empty() {
            return out;
        }
        let mut prev = *poly.last().unwrap();
        let mut prev_inside = inside(&prev);

        for curr in poly {
            let curr_inside = inside(&curr);
            if curr_inside {
                if prev_inside {
                    out.push(curr);
                } else {
                    out.push(intersect(&prev, &curr));
                    out.push(curr);
                }
            } else if prev_inside {
                out.push(intersect(&prev, &curr));
            }
            prev = curr;
            prev_inside = curr_inside;
        }
        out
    };

    let p1 = clip_halfplane(
        pts.to_vec(),
        &|p| p.x >= min_x,
        &|p1, p2| {
            let dx = p2.x - p1.x;
            let t = if dx.abs() < 1e-12 { 0.0 } else { (min_x - p1.x) / dx };
            Point2D::new(min_x, p1.y + t * (p2.y - p1.y))
        },
    );

    let p2 = clip_halfplane(
        p1,
        &|p| p.x <= max_x,
        &|p1, p2| {
            let dx = p2.x - p1.x;
            let t = if dx.abs() < 1e-12 { 0.0 } else { (max_x - p1.x) / dx };
            Point2D::new(max_x, p1.y + t * (p2.y - p1.y))
        },
    );

    let p3 = clip_halfplane(
        p2,
        &|p| p.y >= min_y,
        &|p1, p2| {
            let dy = p2.y - p1.y;
            let t = if dy.abs() < 1e-12 { 0.0 } else { (min_y - p1.y) / dy };
            Point2D::new(p1.x + t * (p2.x - p1.x), min_y)
        },
    );

    let p4 = clip_halfplane(
        p3,
        &|p| p.y <= max_y,
        &|p1, p2| {
            let dy = p2.y - p1.y;
            let t = if dy.abs() < 1e-12 { 0.0 } else { (max_y - p1.y) / dy };
            Point2D::new(p1.x + t * (p2.x - p1.x), max_y)
        },
    );

    let mut clean: Vec<Point2D> = Vec::new();
    for p in p4 {
        let snapped_p = snap_point_to_rect_boundary(p, min_x, min_y, max_x, max_y);
        if clean.is_empty() || clean.last().unwrap().distance_to(&snapped_p) > 1e-6 {
            clean.push(snapped_p);
        }
    }
    if clean.len() >= 3 && clean.first().unwrap().distance_to(clean.last().unwrap()) < 1e-6 {
        clean.pop();
    }

    if clean.len() >= 3 {
        clean
    } else {
        Vec::new()
    }
}

fn clip_path_to_rect(path: &VectorPath, total_w: f64, total_h: f64) -> Option<VectorPath> {
    if path.commands.is_empty() {
        return None;
    }

    let (min_x, min_y, max_x, max_y) = path.bounding_box();

    if max_x <= 0.0 || min_x >= total_w || max_y <= 0.0 || min_y >= total_h {
        return None;
    }

    let eps = 1e-3;
    if min_x >= -eps && max_x <= total_w + eps && min_y >= -eps && max_y <= total_h + eps {
        return Some(path.clone());
    }

    let subpaths = split_subpaths(&path.commands);
    let mut clipped_commands = Vec::new();

    for subpath in subpaths {
        let pts = CurveFlattener::flatten_subpath(&subpath, 0.2);
        let clipped_pts = clip_polygon_to_rect(&pts, 0.0, 0.0, total_w, total_h);

        if clipped_pts.len() >= 3 {
            clipped_commands.push(PathCommand::MoveTo(clipped_pts[0]));
            for pt in &clipped_pts[1..] {
                clipped_commands.push(PathCommand::LineTo(*pt));
            }
            clipped_commands.push(PathCommand::Close);
        }
    }

    if clipped_commands.is_empty() {
        None
    } else {
        let mut clipped_path = VectorPath::new(path.id.clone());
        clipped_path.fill = path.fill.clone();
        clipped_path.stroke = path.stroke.clone();
        clipped_path.stroke_width = path.stroke_width;
        clipped_path.fill_rule = path.fill_rule.clone();
        clipped_path.commands = clipped_commands;
        Some(clipped_path)
    }
}

pub struct PatternGenerator;

impl PatternGenerator {
    /// Generates a seamless tiled pattern SVG `VectorDocument` ONLY if PatternDetector approves.
    pub fn generate_pattern(
        doc: &VectorDocument,
        grid_x: u32,
        grid_y: u32,
    ) -> Result<VectorDocument, VectorCoreError> {
        Self::generate_pattern_full_config(doc, grid_x, grid_y, "grid", 0.0, false, false)
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
            None => {
                let w = doc.width.unwrap_or(0.0);
                let h = doc.height.unwrap_or(0.0);
                if w > 0.0 && h > 0.0 {
                    (w, h)
                } else {
                    let mut max_x = 100.0f64;
                    let mut max_y = 100.0f64;
                    for p in &doc.paths {
                        let (_, _, px2, py2) = p.bounding_box();
                        if px2.is_finite() && px2 > max_x { max_x = px2; }
                        if py2.is_finite() && py2 > max_y { max_y = py2; }
                    }
                    (max_x, max_y)
                }
            }
        };

        let total_w = base_w * grid_x as f64;
        let total_h = base_h * grid_y as f64;

        let mut tiled_doc = VectorDocument::new();
        tiled_doc.width = Some(total_w);
        tiled_doc.height = Some(total_h);
        tiled_doc.view_box = Some((0.0, 0.0, total_w, total_h));

        // 1. Detect background color & filter out tile-level base background rectangles
        let mut detected_bg_color: Option<String> = None;
        let mut cleaned_doc_paths: Vec<VectorPath> = Vec::new();

        for (idx, path) in doc.paths.iter().enumerate() {
            let subpaths = split_subpaths(&path.commands);
            let mut artwork_subpaths: Vec<Vec<PathCommand>> = Vec::new();
            let is_bottom = idx == 0;

            for subpath in subpaths {
                if is_base_background_rect(&subpath, base_w, base_h, is_bottom) {
                    if detected_bg_color.is_none() && path.fill.is_some() {
                        detected_bg_color = path.fill.clone();
                    }
                } else {
                    artwork_subpaths.push(subpath);
                }
            }

            if !artwork_subpaths.is_empty() {
                let mut clean_path = path.clone();
                clean_path.commands = artwork_subpaths.into_iter().flatten().collect();
                cleaned_doc_paths.push(clean_path);
            }
        }

        if detected_bg_color.is_none() && !doc.paths.is_empty() {
            if let Some(ref first_fill) = doc.paths[0].fill {
                if first_fill != "none" {
                    detected_bg_color = Some(first_fill.clone());
                    let p0_bounds = doc.paths[0].bounding_box();
                    let p0_w = p0_bounds.2 - p0_bounds.0;
                    let p0_h = p0_bounds.3 - p0_bounds.1;
                    if p0_w >= base_w * 0.95 && p0_h >= base_h * 0.95 && !cleaned_doc_paths.is_empty() {
                        cleaned_doc_paths.remove(0);
                    }
                }
            }
        }

        // 2. Insert Layer 1 Full-Canvas Background Rectangle at the bottom (index 0) if a background color was detected
        if let Some(ref bg_color) = detected_bg_color {
            let mut full_bg_path = VectorPath::new(Some("pattern_background_layer_1".to_string()));
            full_bg_path.fill = Some(bg_color.clone());
            full_bg_path.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
            full_bg_path.add_command(PathCommand::LineTo(Point2D::new(total_w, 0.0)));
            full_bg_path.add_command(PathCommand::LineTo(Point2D::new(total_w, total_h)));
            full_bg_path.add_command(PathCommand::LineTo(Point2D::new(0.0, total_h)));
            full_bg_path.add_command(PathCommand::Close);
            tiled_doc.paths.push(full_bg_path);
        }

        let style_lower = style.to_lowercase();
        
        let is_slanted_left = style_lower.contains("slanted_left") || style_lower.contains("miring_kiri");
        let is_slanted_right = style_lower.contains("slanted_right") || style_lower.contains("miring_kanan");
        let is_slanted_general = is_slanted_left || is_slanted_right || style_lower.eq("slanted") || style_lower.eq("miring");
        
        let is_half_drop_v = style_lower.contains("half_drop_vertical") || style_lower.contains("half_drop_v") || style_lower.contains("column_stagger");
        let is_brick_h = style_lower.contains("brick") || style_lower.contains("half_drop_horizontal") || style_lower.contains("half_drop_h");
        let is_diamond = style_lower.contains("diamond");
        let is_pinwheel = style_lower.contains("pinwheel") || style_lower.contains("kincir") || style_lower.contains("checker_pinwheel");
        let is_scale_rhythm = style_lower.contains("scale_rhythm") || style_lower.contains("dual_scale");
        let is_sine_wave = style_lower.contains("sine") || style_lower.contains("wave");
        let is_kaleidoscope = style_lower.contains("kaleidoscope") || style_lower.contains("quad_mirror");
        let is_alternating = style_lower.contains("alternating") || style_lower.contains("flipped") || style_lower.contains("bolak_balik");

        let mut candidate_paths: Vec<VectorPath> = Vec::new();

        // Extended grid coverage for offset layout styles
        let extra_cols = if is_slanted_general || is_brick_h || is_diamond || is_scale_rhythm || is_sine_wave { 2 } else { 0 };
        let extra_rows = if is_half_drop_v || is_diamond { 2 } else { 0 };

        let col_start = 0 - extra_cols;
        let col_end = grid_x as i32 + extra_cols;
        let row_start = 0 - extra_rows;
        let row_end = grid_y as i32 + extra_rows;

        for r in row_start..row_end {
            for c in col_start..col_end {
                let (dx, dy) = if is_slanted_left {
                    (c as f64 * base_w + r as f64 * (base_w * slant_offset_ratio), r as f64 * base_h)
                } else if is_slanted_right {
                    (c as f64 * base_w + r as f64 * (base_w * slant_offset_ratio.abs()), r as f64 * base_h)
                } else if is_half_drop_v {
                    (c as f64 * base_w, r as f64 * base_h + (c.abs() % 2) as f64 * (0.5 * base_h))
                } else if is_brick_h {
                    (c as f64 * base_w + (r.abs() % 2) as f64 * (0.5 * base_w), r as f64 * base_h)
                } else if is_diamond {
                    (
                        c as f64 * base_w + (r.abs() % 2) as f64 * (0.5 * base_w),
                        r as f64 * base_h + (c.abs() % 2) as f64 * (0.5 * base_h),
                    )
                } else if is_scale_rhythm {
                    (c as f64 * base_w + (r.abs() % 2) as f64 * (0.5 * base_w), r as f64 * base_h)
                } else if is_sine_wave {
                    let offset = (r as f64 * std::f64::consts::FRAC_PI_2).sin() * (0.5 * base_w);
                    (c as f64 * base_w + offset, r as f64 * base_h)
                } else {
                    (c as f64 * base_w, r as f64 * base_h)
                };

                let flip_tile = is_alternating && ((r.abs() + c.abs()) % 2 == 1);
                let pinwheel_angle = if is_pinwheel {
                    (((r.abs() % 4 + c.abs() % 4) % 4) as f64) * 90.0
                } else {
                    0.0
                };
                let scale_factor = if is_scale_rhythm && (r.abs() % 2 == 1) { 0.75 } else { 1.0 };
                let k_flip_x = is_kaleidoscope && (c.abs() % 2 == 1);
                let k_flip_y = is_kaleidoscope && (r.abs() % 2 == 1);

                let tile_center = Point2D::new(dx + 0.5 * base_w, dy + 0.5 * base_h);

                for path in &cleaned_doc_paths {
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
                        if is_kaleidoscope {
                            let px = if k_flip_x { base_w - p.x } else { p.x };
                            let py = if k_flip_y { base_h - p.y } else { p.y };
                            Point2D::new(dx + px, dy + py)
                        } else if flip_tile {
                            Point2D::new(dx + (base_w - p.x), dy + (base_h - p.y))
                        } else {
                            let base_pt = Point2D::new(p.x + dx, p.y + dy);
                            let scaled_pt = if (scale_factor - 1.0f64).abs() > 1e-4 {
                                Point2D::new(
                                    tile_center.x + (base_pt.x - tile_center.x) * scale_factor,
                                    tile_center.y + (base_pt.y - tile_center.y) * scale_factor,
                                )
                            } else {
                                base_pt
                            };

                            if pinwheel_angle > 0.0 {
                                let rad = pinwheel_angle.to_radians();
                                let cos_a = rad.cos();
                                let sin_a = rad.sin();
                                let rx = scaled_pt.x - tile_center.x;
                                let ry = scaled_pt.y - tile_center.y;
                                Point2D::new(
                                    tile_center.x + rx * cos_a - ry * sin_a,
                                    tile_center.y + rx * sin_a + ry * cos_a,
                                )
                            } else {
                                scaled_pt
                            }
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

        let mut unclipped_candidates: Vec<VectorPath> = Vec::new();

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
                unclipped_candidates.push(path.clone());
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
                    unclipped_candidates.push(wrapped_path);
                }
            }
        }

        // Geometric Clipping Pass: Parallel multi-core path clipping across all CPU cores
        use rayon::prelude::*;
        let clipped_paths: Vec<VectorPath> = unclipped_candidates
            .into_par_iter()
            .filter_map(|path| clip_path_to_rect(&path, total_w, total_h))
            .collect();

        tiled_doc.paths.extend(clipped_paths);

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

        // 3x3 grid of 1 path = 1 full-canvas background + 9 tiled paths = 10 paths total
        assert_eq!(tiled_doc.paths.len(), 10);

        // Check color preservation on all artwork tiles
        for tile in &tiled_doc.paths[1..] {
            assert_eq!(tile.fill.as_deref(), Some("#FF5722"));
            assert_eq!(tile.stroke.as_deref(), Some("#212121"));
            assert_eq!(tile.stroke_width, Some(1.5));
        }

        assert_eq!(tiled_doc.view_box, Some((0.0, 0.0, 300.0, 300.0)));
    }

    #[test]
    fn test_creative_pattern_styles() {
        let mut doc = VectorDocument::new();
        doc.view_box = Some((0.0, 0.0, 100.0, 100.0));

        let mut path = VectorPath::new(Some("motif".to_string()));
        path.fill = Some("#4CAF50".to_string());
        path.add_command(PathCommand::MoveTo(Point2D::new(20.0, 20.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(80.0, 20.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(80.0, 80.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(20.0, 80.0)));
        path.add_command(PathCommand::Close);
        doc.paths.push(path);

        let styles = vec![
            "half_drop_vertical",
            "brick_wall",
            "diamond_stagger",
            "checker_pinwheel",
            "scale_rhythm",
            "wave_sine_offset",
            "kaleidoscope_mirror",
        ];

        for style in styles {
            let res = PatternGenerator::generate_pattern_full_config(&doc, 3, 3, style, -0.5, true, false);
            assert!(res.is_ok(), "Failed to generate pattern for style: {}", style);
            let tiled = res.unwrap();
            assert!(!tiled.paths.is_empty(), "Generated pattern paths empty for style: {}", style);
            assert_eq!(tiled.view_box, Some((0.0, 0.0, 300.0, 300.0)));

            // Bounding box assertion: zero points outside 0..300
            for path in &tiled.paths {
                let (min_x, min_y, max_x, max_y) = path.bounding_box();
                assert!(min_x >= -1e-3, "Style {} min_x < 0: {}", style, min_x);
                assert!(max_x <= 300.0 + 1e-3, "Style {} max_x > 300: {}", style, max_x);
                assert!(min_y >= -1e-3, "Style {} min_y < 0: {}", style, min_y);
                assert!(max_y <= 300.0 + 1e-3, "Style {} max_y > 300: {}", style, max_y);
            }
        }
    }
}
