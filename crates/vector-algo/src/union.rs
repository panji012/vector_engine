use vector_core::{Point2D, Polygon2D, VectorCoreError};

/// Polygon Boolean Union operator.
pub struct PolygonUnion;

impl PolygonUnion {
    /// Computes the Union of two 2D Polygons.
    pub fn union(poly1: &Polygon2D, poly2: &Polygon2D) -> Result<Vec<Polygon2D>, VectorCoreError> {
        if poly1.exterior.is_empty() {
            return Ok(vec![poly2.clone()]);
        }
        if poly2.exterior.is_empty() {
            return Ok(vec![poly1.clone()]);
        }

        // Bounding box check for fast disjoint test
        let bb1 = bounding_box(&poly1.exterior);
        let bb2 = bounding_box(&poly2.exterior);

        if !bboxes_overlap(&bb1, &bb2) {
            // Disjoint polygons
            return Ok(vec![poly1.clone(), poly2.clone()]);
        }

        // Check if poly1 contains poly2 or vice versa
        if polygon_contains_polygon(poly1, poly2) {
            return Ok(vec![poly1.clone()]);
        }
        if polygon_contains_polygon(poly2, poly1) {
            return Ok(vec![poly2.clone()]);
        }

        // Compute polygon clipping/union vertex merging
        let merged_exterior = merge_polygon_exteriors(&poly1.exterior, &poly2.exterior)?;
        Ok(vec![Polygon2D::new(merged_exterior)])
    }
}

fn bounding_box(ring: &[Point2D]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for pt in ring {
        if pt.x < min_x { min_x = pt.x; }
        if pt.y < min_y { min_y = pt.y; }
        if pt.x > max_x { max_x = pt.x; }
        if pt.y > max_y { max_y = pt.y; }
    }
    (min_x, min_y, max_x, max_y)
}

fn bboxes_overlap(b1: &(f64, f64, f64, f64), b2: &(f64, f64, f64, f64)) -> bool {
    !(b1.2 < b2.0 || b2.2 < b1.0 || b1.3 < b2.1 || b2.3 < b1.1)
}

fn point_in_polygon(pt: &Point2D, ring: &[Point2D]) -> bool {
    let mut inside = false;
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let pi = &ring[i];
        let pj = &ring[j];
        if ((pi.y > pt.y) != (pj.y > pt.y))
            && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y + 1e-12) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn polygon_contains_polygon(outer: &Polygon2D, inner: &Polygon2D) -> bool {
    for pt in &inner.exterior {
        if !point_in_polygon(pt, &outer.exterior) {
            return false;
        }
    }
    true
}

/// Merges exterior boundary rings of two overlapping polygons into a single boundary.
fn merge_polygon_exteriors(ring1: &[Point2D], ring2: &[Point2D]) -> Result<Vec<Point2D>, VectorCoreError> {
    let mut points = Vec::new();

    for pt in ring1 {
        if !point_in_polygon(pt, ring2) {
            points.push(*pt);
        }
    }

    for pt in ring2 {
        if !point_in_polygon(pt, ring1) {
            points.push(*pt);
        }
    }

    if points.len() < 3 {
        let mut all_pts = ring1.to_vec();
        all_pts.extend_from_slice(ring2);
        return Ok(convex_hull(&all_pts));
    }

    let cx = points.iter().map(|p| p.x).sum::<f64>() / points.len() as f64;
    let cy = points.iter().map(|p| p.y).sum::<f64>() / points.len() as f64;

    points.sort_by(|a, b| {
        let angle_a = (a.y - cy).atan2(a.x - cx);
        let angle_b = (b.y - cy).atan2(b.x - cx);
        angle_a.partial_cmp(&angle_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(points)
}

fn convex_hull(points: &[Point2D]) -> Vec<Point2D> {
    if points.len() <= 3 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap().then_with(|| a.y.partial_cmp(&b.y).unwrap()));

    let mut lower = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross_product(&lower[lower.len() - 2], &lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }

    let mut upper = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross_product(&upper[upper.len() - 2], &upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn cross_product(o: &Point2D, a: &Point2D, b: &Point2D) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disjoint_union() {
        let p1 = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(2.0, 0.0),
            Point2D::new(2.0, 2.0), Point2D::new(0.0, 2.0)
        ]);
        let p2 = Polygon2D::new(vec![
            Point2D::new(10.0, 10.0), Point2D::new(12.0, 10.0),
            Point2D::new(12.0, 12.0), Point2D::new(10.0, 12.0)
        ]);

        let result = PolygonUnion::union(&p1, &p2).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_overlapping_union() {
        let p1 = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(10.0, 0.0),
            Point2D::new(10.0, 10.0), Point2D::new(0.0, 10.0)
        ]);
        let p2 = Polygon2D::new(vec![
            Point2D::new(5.0, 0.0), Point2D::new(15.0, 0.0),
            Point2D::new(15.0, 10.0), Point2D::new(5.0, 10.0)
        ]);

        let result = PolygonUnion::union(&p1, &p2).unwrap();
        assert_eq!(result.len(), 1);
        let union_area = result[0].area();
        assert!((union_area - 150.0).abs() < 1e-5);
    }

    #[test]
    fn test_contained_union() {
        let outer = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(20.0, 0.0),
            Point2D::new(20.0, 20.0), Point2D::new(0.0, 20.0)
        ]);
        let inner = Polygon2D::new(vec![
            Point2D::new(5.0, 5.0), Point2D::new(10.0, 5.0),
            Point2D::new(10.0, 10.0), Point2D::new(5.0, 10.0)
        ]);

        let result = PolygonUnion::union(&outer, &inner).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].area(), 400.0);
    }
}
