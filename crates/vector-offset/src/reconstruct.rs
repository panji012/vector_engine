use crate::polygon::PolygonRing;
use crate::topology::RingRole;
use vector_core::{PathCommand, Point2D, VectorPath};

pub struct PathReconstructor;

impl PathReconstructor {
    /// Reconstructs SVG compound paths with RDP node simplification (epsilon = 0.2 px), smooth cubic Bézier curve fitting (`CubicTo`), and fill-rule="evenodd".
    pub fn reconstruct_svg_path(
        contours: &[(PolygonRing, RingRole)],
        original_path: &VectorPath,
        enable_simplify: bool,
        simplify_tolerance: f64,
    ) -> VectorPath {
        let mut new_path = VectorPath::new(original_path.id.clone());
        new_path.fill = original_path.fill.clone();
        new_path.stroke = original_path.stroke.clone();
        new_path.stroke_width = original_path.stroke_width;
        new_path.fill_rule = Some("evenodd".to_string());

        for (ring, role) in contours {
            if ring.vertices.is_empty() {
                continue;
            }

            // Enforce winding orientation: Exterior rings CW (area > 0), Hole rings CCW (area < 0)
            let mut oriented_verts = ring.vertices.clone();
            let is_cw = ring.signed_area() > 0.0;

            match role {
                RingRole::Exterior => {
                    if !is_cw {
                        oriented_verts.reverse();
                    }
                }
                RingRole::Hole => {
                    if is_cw {
                        oriented_verts.reverse();
                    }
                }
            }

            // 1. Apply RDP Node Simplification Pass if enabled
            let pts_to_use = if enable_simplify {
                let simplified_pts = rdp_simplify(&oriented_verts, simplify_tolerance);
                if simplified_pts.is_empty() { oriented_verts.clone() } else { simplified_pts }
            } else {
                oriented_verts.clone()
            };

            // 2. Fit smooth Bézier curves & line commands
            let commands = fit_bezier_curves(&pts_to_use, ring.is_closed, 0.5);
            for cmd in commands {
                new_path.add_command(cmd);
            }
        }

        new_path
    }

    /// Combines calculated contours of identical fill color into sequential compound paths with `fill-rule="evenodd"`.
    ///
    /// Strictly preserves original SVG DOM Z-ordering (draw order) without area-based sorting.
    pub fn combine_contours_by_color(
        color_contour_pairs: Vec<(Option<String>, Vec<(PolygonRing, RingRole)>)>,
        enable_simplify: bool,
        simplify_tolerance: f64,
    ) -> Vec<VectorPath> {
        let mut compound_paths = Vec::new();
        
        // Iterating sequentially and combining adjacent identical colors strictly preserves original draw order
        let mut current_color: Option<Option<String>> = None;
        let mut current_contours: Vec<(PolygonRing, RingRole)> = Vec::new();
        
        for (color, contours) in color_contour_pairs {
            if let Some(ref c) = current_color {
                if *c == color {
                    current_contours.extend(contours);
                    continue;
                } else {
                    // Finalize current path
                    if !current_contours.is_empty() {
                        compound_paths.push(Self::build_compound_path(c.clone(), &current_contours, enable_simplify, simplify_tolerance));
                    }
                    current_contours.clear();
                }
            }
            current_color = Some(color);
            current_contours.extend(contours);
        }
        
        if let Some(c) = current_color {
            if !current_contours.is_empty() {
                compound_paths.push(Self::build_compound_path(c, &current_contours, enable_simplify, simplify_tolerance));
            }
        }
        
        compound_paths
    }

    fn build_compound_path(color: Option<String>, contours: &[(PolygonRing, RingRole)], enable_simplify: bool, simplify_tolerance: f64) -> VectorPath {
        let color_key = color.clone().unwrap_or_else(|| "none".to_string());
        let path_id = format!("group_{}", color_key.replace('#', "hex_"));

        let mut compound_path = VectorPath::new(Some(path_id));
        compound_path.fill = color;
        compound_path.fill_rule = Some("evenodd".to_string());

        for (ring, role) in contours {
            // Filter out degenerate rings (e.g. orphan MoveTo points) that distort bounds
            if ring.vertices.len() < 3 {
                continue;
            }

            // Enforce winding orientation: Exterior rings CW (area > 0), Hole rings CCW (area < 0)
            let mut oriented_verts = ring.vertices.clone();
            let is_cw = ring.signed_area() > 0.0;

            match role {
                RingRole::Exterior => {
                    if !is_cw {
                        oriented_verts.reverse();
                    }
                }
                RingRole::Hole => {
                    if is_cw {
                        oriented_verts.reverse();
                    }
                }
            }

            // 1. Apply RDP Node Simplification Pass if enabled
            let pts_to_use = if enable_simplify {
                let simplified_pts = rdp_simplify(&oriented_verts, simplify_tolerance);
                if simplified_pts.is_empty() { oriented_verts.clone() } else { simplified_pts }
            } else {
                oriented_verts.clone()
            };

            // 2. Fit smooth Bézier curves & line commands
            let commands = fit_bezier_curves(&pts_to_use, ring.is_closed, 0.5);
            for cmd in commands {
                compound_path.add_command(cmd);
            }
        }

        compound_path
    }
}

/// Ramer-Douglas-Peucker (RDP) polyline simplification algorithm (epsilon = 0.2 px).
pub fn rdp_simplify(pts: &[Point2D], epsilon: f64) -> Vec<Point2D> {
    let n = pts.len();
    if n <= 2 || epsilon <= 0.0 {
        return pts.to_vec();
    }

    let mut out = Vec::new();
    rdp_recursive(pts, 0, n - 1, epsilon, &mut out);
    out.push(pts[n - 1]);

    if out.len() >= 3 && out.first().unwrap().distance_to(out.last().unwrap()) < 1e-6 {
        out.pop();
    }
    out
}

fn rdp_recursive(pts: &[Point2D], start: usize, end: usize, epsilon: f64, out: &mut Vec<Point2D>) {
    if end <= start + 1 {
        out.push(pts[start]);
        return;
    }

    let mut dmax = 0.0;
    let mut index = start;

    let a = pts[start];
    let b = pts[end];

    for i in (start + 1)..end {
        let d = perpendicular_distance(pts[i], a, b);
        if d > dmax {
            dmax = d;
            index = i;
        }
    }

    if dmax > epsilon {
        rdp_recursive(pts, start, index, epsilon, out);
        rdp_recursive(pts, index, end, epsilon, out);
    } else {
        out.push(pts[start]);
    }
}

fn perpendicular_distance(p: Point2D, a: Point2D, b: Point2D) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let len = (ab_x * ab_x + ab_y * ab_y).sqrt();
    if len < 1e-12 {
        return p.distance_to(&a);
    }
    ((p.x - a.x) * ab_y - (p.y - a.y) * ab_x).abs() / len
}

/// Fits adaptive line segments and smooth cubic Bézier curves (CubicTo) to a polyline ring.
fn fit_bezier_curves(pts: &[Point2D], is_closed: bool, max_error: f64) -> Vec<PathCommand> {
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }
    if n < 3 {
        let mut cmds = vec![PathCommand::MoveTo(pts[0])];
        for p in &pts[1..] {
            cmds.push(PathCommand::LineTo(*p));
        }
        if is_closed {
            cmds.push(PathCommand::Close);
        }
        return cmds;
    }

    let mut cmds = Vec::new();
    cmds.push(PathCommand::MoveTo(pts[0]));

    let mut i = 0;
    while i < n - 1 {
        let mut best_j = i + 1;
        let mut best_cubic: Option<(Point2D, Point2D, Point2D)> = None;

        let max_lookahead = (i + 12).min(n - 1);
        for j in (i + 2)..=max_lookahead {
            let sub = &pts[i..=j];
            if is_line_segment(sub, 0.05) {
                best_j = j;
                best_cubic = None;
                continue;
            }

            if let Some((c1, c2, end)) = try_fit_cubic(sub, max_error) {
                best_j = j;
                best_cubic = Some((c1, c2, end));
            } else {
                break;
            }
        }

        if let Some((c1, c2, end)) = best_cubic {
            cmds.push(PathCommand::CubicTo { control1: c1, control2: c2, end });
            i = best_j;
        } else if best_j > i + 1 {
            cmds.push(PathCommand::LineTo(pts[best_j]));
            i = best_j;
        } else {
            cmds.push(PathCommand::LineTo(pts[i + 1]));
            i += 1;
        }
    }

    if is_closed {
        cmds.push(PathCommand::Close);
    }

    cmds
}

fn is_line_segment(pts: &[Point2D], max_error: f64) -> bool {
    let n = pts.len();
    if n <= 2 {
        return true;
    }
    let a = pts[0];
    let b = pts[n - 1];
    let len = a.distance_to(&b);
    if len < 1e-6 {
        return false;
    }

    for p in &pts[1..n - 1] {
        let ab_x = b.x - a.x;
        let ab_y = b.y - a.y;
        let dist = ((p.x - a.x) * ab_y - (p.y - a.y) * ab_x).abs() / len;
        if dist > max_error {
            return false;
        }
    }
    true
}

fn try_fit_cubic(pts: &[Point2D], max_error: f64) -> Option<(Point2D, Point2D, Point2D)> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    let p0 = pts[0];
    let p3 = pts[n - 1];
    let dist_chord = p0.distance_to(&p3);
    if dist_chord < 1e-6 {
        return None;
    }

    let v_start = Point2D::new(pts[1].x - p0.x, pts[1].y - p0.y);
    let v_end = Point2D::new(p3.x - pts[n - 2].x, p3.y - pts[n - 2].y);

    let len_start = (v_start.x * v_start.x + v_start.y * v_start.y).sqrt().max(1e-9);
    let len_end = (v_end.x * v_end.x + v_end.y * v_end.y).sqrt().max(1e-9);

    let u_start = Point2D::new(v_start.x / len_start, v_start.y / len_start);
    let u_end = Point2D::new(v_end.x / len_end, v_end.y / len_end);

    let alpha = dist_chord / 3.0;

    let c1 = Point2D::new(p0.x + u_start.x * alpha, p0.y + u_start.y * alpha);
    let c2 = Point2D::new(p3.x - u_end.x * alpha, p3.y - u_end.y * alpha);

    for (k, p) in pts.iter().enumerate().take(n - 1).skip(1) {
        let t = k as f64 / (n - 1) as f64;
        let b = eval_cubic_bezier(p0, c1, c2, p3, t);
        if p.distance_to(&b) > max_error {
            return None;
        }
    }

    Some((c1, c2, p3))
}

fn eval_cubic_bezier(p0: Point2D, c1: Point2D, c2: Point2D, p3: Point2D, t: f64) -> Point2D {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    let x = uuu * p0.x + 3.0 * uu * t * c1.x + 3.0 * u * tt * c2.x + ttt * p3.x;
    let y = uuu * p0.y + 3.0 * uu * t * c1.y + 3.0 * u * tt * c2.y + ttt * p3.y;

    Point2D::new(x, y)
}
