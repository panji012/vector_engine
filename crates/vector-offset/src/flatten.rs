use vector_core::{PathCommand, Point2D};

/// Adaptive Bézier curve flattener into polyline vertices using chord-distance flatness tolerance.
pub struct CurveFlattener;

impl CurveFlattener {
    /// Flattens SVG path commands into adaptive polyline points based on chord-distance tolerance.
    pub fn flatten_subpath(commands: &[PathCommand], tolerance: f64) -> Vec<Point2D> {
        if commands.is_empty() {
            return Vec::new();
        }

        let tol = if tolerance <= 0.0 { 0.1 } else { tolerance };
        let mut pts = Vec::new();
        let mut curr = Point2D::new(0.0, 0.0);

        for cmd in commands {
            match cmd {
                PathCommand::MoveTo(pt) => {
                    curr = *pt;
                    pts.push(*pt);
                }
                PathCommand::LineTo(pt) => {
                    curr = *pt;
                    pts.push(*pt);
                }
                PathCommand::QuadTo { control, end } => {
                    Self::flatten_quad_adaptive(curr, *control, *end, tol, &mut pts, 0);
                    curr = *end;
                }
                PathCommand::CubicTo { control1, control2, end } => {
                    Self::flatten_cubic_adaptive(curr, *control1, *control2, *end, tol, &mut pts, 0);
                    curr = *end;
                }
                PathCommand::Close => {}
            }
        }

        // Deduplicate consecutive points within 1e-9 tolerance
        let mut clean: Vec<Point2D> = Vec::new();
        for p in pts {
            if clean.is_empty() || clean.last().unwrap().distance_to(&p) > 1e-9 {
                clean.push(p);
            }
        }
        clean
    }

    fn flatten_quad_adaptive(p0: Point2D, c: Point2D, p1: Point2D, tol: f64, out: &mut Vec<Point2D>, depth: usize) {
        let d = dist_point_to_line(c, p0, p1);
        if d <= tol || depth >= 10 {
            out.push(p1);
        } else {
            // de Casteljau split at t = 0.5
            let p01 = Point2D::new((p0.x + c.x) * 0.5, (p0.y + c.y) * 0.5);
            let p11 = Point2D::new((c.x + p1.x) * 0.5, (c.y + p1.y) * 0.5);
            let p02 = Point2D::new((p01.x + p11.x) * 0.5, (p01.y + p11.y) * 0.5);

            Self::flatten_quad_adaptive(p0, p01, p02, tol, out, depth + 1);
            Self::flatten_quad_adaptive(p02, p11, p1, tol, out, depth + 1);
        }
    }

    fn flatten_cubic_adaptive(p0: Point2D, c1: Point2D, c2: Point2D, p1: Point2D, tol: f64, out: &mut Vec<Point2D>, depth: usize) {
        let d1 = dist_point_to_line(c1, p0, p1);
        let d2 = dist_point_to_line(c2, p0, p1);
        let d = d1.max(d2);

        if d <= tol || depth >= 10 {
            out.push(p1);
        } else {
            // de Casteljau split at t = 0.5
            let p01 = Point2D::new((p0.x + c1.x) * 0.5, (p0.y + c1.y) * 0.5);
            let p12 = Point2D::new((c1.x + c2.x) * 0.5, (c1.y + c2.y) * 0.5);
            let p23 = Point2D::new((c2.x + p1.x) * 0.5, (c2.y + p1.y) * 0.5);

            let p012 = Point2D::new((p01.x + p12.x) * 0.5, (p01.y + p12.y) * 0.5);
            let p123 = Point2D::new((p12.x + p23.x) * 0.5, (p12.y + p23.y) * 0.5);
            let p0123 = Point2D::new((p012.x + p123.x) * 0.5, (p012.y + p123.y) * 0.5);

            Self::flatten_cubic_adaptive(p0, p01, p012, p0123, tol, out, depth + 1);
            Self::flatten_cubic_adaptive(p0123, p123, p23, p1, tol, out, depth + 1);
        }
    }
}

fn dist_point_to_line(p: Point2D, a: Point2D, b: Point2D) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let len_sq = ab_x * ab_x + ab_y * ab_y;
    if len_sq < 1e-12 {
        return p.distance_to(&a);
    }
    let cross = ((p.x - a.x) * ab_y - (p.y - a.y) * ab_x).abs();
    cross / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_cubic_flattening() {
        let cmds = vec![
            PathCommand::MoveTo(Point2D::new(0.0, 0.0)),
            PathCommand::CubicTo {
                control1: Point2D::new(25.0, -10.0),
                control2: Point2D::new(75.0, -10.0),
                end: Point2D::new(100.0, 0.0),
            },
        ];

        let pts = CurveFlattener::flatten_subpath(&cmds, 0.1);
        assert!(pts.len() > 4);
        assert_eq!(pts[0], Point2D::new(0.0, 0.0));
        assert_eq!(*pts.last().unwrap(), Point2D::new(100.0, 0.0));
    }
}
