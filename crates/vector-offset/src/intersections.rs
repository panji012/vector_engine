use crate::polygon::{PolygonRing, Segment2D};
use vector_core::Point2D;

/// Represents an intersection point between two line segments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntersectionPoint {
    pub point: Point2D,
    pub seg1_index: usize,
    pub seg2_index: usize,
    pub t1: f64,
    pub t2: f64,
}

pub struct SelfIntersectionDetector;

impl SelfIntersectionDetector {
    /// Detects self-intersections within a polygon ring with a safety iteration limit.
    pub fn find_self_intersections(ring: &PolygonRing) -> Vec<IntersectionPoint> {
        let segs = ring.segments();
        let n = segs.len();
        if n < 4 {
            return Vec::new();
        }

        let mut intersections = Vec::new();
        const MAX_INTERSECTIONS: usize = 100;
        let mut total_checks = 0;
        const MAX_PAIR_CHECKS: usize = 50_000;

        for i in 0..n {
            for j in (i + 2)..n {
                total_checks += 1;
                if total_checks >= MAX_PAIR_CHECKS {
                    return intersections; // Guard against quadratic pair check hangs
                }

                // Ignore adjacent segments for closed ring (0 and n-1)
                if i == 0 && j == n - 1 {
                    continue;
                }

                if let Some((pt, t1, t2)) = segment_intersection(&segs[i], &segs[j]) {
                    if t1 > 1e-6 && t1 < (1.0 - 1e-6) && t2 > 1e-6 && t2 < (1.0 - 1e-6) {
                        intersections.push(IntersectionPoint {
                            point: pt,
                            seg1_index: i,
                            seg2_index: j,
                            t1,
                            t2,
                        });
                        if intersections.len() >= MAX_INTERSECTIONS {
                            return intersections; // Guard against self-intersection explosion
                        }
                    }
                }
            }
        }

        intersections
    }
}

pub fn segment_intersection(s1: &Segment2D, s2: &Segment2D) -> Option<(Point2D, f64, f64)> {
    let p = s1.start;
    let r = Point2D::new(s1.end.x - s1.start.x, s1.end.y - s1.start.y);
    let q = s2.start;
    let s = Point2D::new(s2.end.x - s2.start.x, s2.end.y - s2.start.y);

    let r_cross_s = r.x * s.y - r.y * s.x;
    let q_minus_p = Point2D::new(q.x - p.x, q.y - p.y);
    let q_minus_p_cross_r = q_minus_p.x * r.y - q_minus_p.y * r.x;

    if r_cross_s.abs() < 1e-12 {
        return None;
    }

    let t = (q_minus_p.x * s.y - q_minus_p.y * s.x) / r_cross_s;
    let u = q_minus_p_cross_r / r_cross_s;

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        let intersection_pt = Point2D::new(p.x + t * r.x, p.y + t * r.y);
        Some((intersection_pt, t, u))
    } else {
        None
    }
}
