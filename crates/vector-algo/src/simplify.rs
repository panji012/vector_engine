use vector_core::{Point2D, Polygon2D, VectorCoreError};

/// Geometry Simplifier implementing Ramer-Douglas-Peucker (RDP) algorithm.
pub struct GeometrySimplifier;

impl GeometrySimplifier {
    /// Simplifies a 2D line segment / polyline using configurable tolerance distance.
    pub fn simplify_points(points: &[Point2D], tolerance: f64) -> Vec<Point2D> {
        if points.len() <= 2 {
            return points.to_vec();
        }

        let mut max_dist = 0.0;
        let mut max_idx = 0;
        let start = points[0];
        let end = points[points.len() - 1];

        for i in 1..(points.len() - 1) {
            let dist = perpendicular_distance(&points[i], &start, &end);
            if dist > max_dist {
                max_dist = dist;
                max_idx = i;
            }
        }

        if max_dist > tolerance {
            let mut left = Self::simplify_points(&points[..=max_idx], tolerance);
            let right = Self::simplify_points(&points[max_idx..], tolerance);

            left.pop(); // Remove duplicate midpoint
            left.extend(right);
            left
        } else {
            vec![start, end]
        }
    }

    /// Simplifies a 2D Polygon exterior ring and interior holes while preserving topology.
    pub fn simplify_polygon(polygon: &Polygon2D, tolerance: f64) -> Result<Polygon2D, VectorCoreError> {
        if polygon.exterior.len() < 3 {
            return Ok(polygon.clone());
        }

        let mut simplified_ext = Self::simplify_points(&polygon.exterior, tolerance);
        // Ensure polygon exterior ring remains closed and valid
        if simplified_ext.len() < 3 {
            simplified_ext = polygon.exterior.clone();
        } else if simplified_ext.first() != simplified_ext.last() {
            let first = simplified_ext[0];
            simplified_ext.push(first);
        }

        let mut simplified_holes = Vec::new();
        for hole in &polygon.interiors {
            let mut s_hole = Self::simplify_points(hole, tolerance);
            if s_hole.len() >= 3 {
                if s_hole.first() != s_hole.last() {
                    let first = s_hole[0];
                    s_hole.push(first);
                }
                simplified_holes.push(s_hole);
            }
        }

        Ok(Polygon2D::with_holes(simplified_ext, simplified_holes))
    }
}

/// Calculates perpendicular distance from point P to line segment AB.
fn perpendicular_distance(p: &Point2D, a: &Point2D, b: &Point2D) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;

    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return p.distance_to(a);
    }

    let num = ((dy * p.x) - (dx * p.y) + (b.x * a.y) - (b.y * a.x)).abs();
    num / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_line_simplification() {
        // Collinear line with slight noise
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.1),
            Point2D::new(2.0, -0.05),
            Point2D::new(3.0, 0.0),
            Point2D::new(4.0, 0.05),
            Point2D::new(5.0, 0.0),
        ];

        let simplified = GeometrySimplifier::simplify_points(&points, 0.2);
        // Should reduce 6 points down to 2 endpoints
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0], Point2D::new(0.0, 0.0));
        assert_eq!(simplified[1], Point2D::new(5.0, 0.0));
    }

    #[test]
    fn test_polygon_simplification_preserves_shape() {
        let poly = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(5.0, 0.02),
            Point2D::new(10.0, 0.0),
            Point2D::new(10.0, 10.0),
            Point2D::new(0.0, 10.0),
            Point2D::new(0.0, 0.0),
        ]);

        let simplified = GeometrySimplifier::simplify_polygon(&poly, 0.1).unwrap();
        // Should eliminate the noisy point (5.0, 0.02)
        assert!(simplified.exterior.len() < poly.exterior.len());
        // Area should remain virtually identical (100.0)
        assert!((simplified.area() - 100.0).abs() < 1e-3);
    }
}
