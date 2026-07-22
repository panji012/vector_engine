use crate::intersections::SelfIntersectionDetector;
use crate::polygon::PolygonRing;

pub struct BooleanResolver;

impl BooleanResolver {
    /// Resolves self-intersections in an offset polygon ring by retaining the main polygon body
    /// and trimming self-intersecting corner loops. Includes safety guards against infinite loops.
    pub fn resolve_self_intersections(ring: &PolygonRing) -> Vec<PolygonRing> {
        let intersections = SelfIntersectionDetector::find_self_intersections(ring);
        if intersections.is_empty() {
            return vec![ring.clone()];
        }

        let verts = &ring.vertices;
        let n = verts.len();
        if n < 4 {
            return vec![ring.clone()];
        }

        let first_intersect = intersections[0];

        // Loop 1: vertices before seg1 + intersection point + vertices after seg2
        let mut loop1_pts = Vec::with_capacity(n);
        const MAX_BODY_POINTS: usize = 10_000;
        let mut steps1 = 0;

        for (idx, v) in verts.iter().enumerate() {
            steps1 += 1;
            if steps1 > MAX_BODY_POINTS {
                break;
            }

            if idx <= first_intersect.seg1_index {
                loop1_pts.push(*v);
            }
            if idx == first_intersect.seg1_index {
                loop1_pts.push(first_intersect.point);
            }
            if idx > first_intersect.seg2_index {
                loop1_pts.push(*v);
            }
        }

        // Loop 2: vertices between seg1 and seg2 + intersection point
        let mut loop2_pts = Vec::with_capacity(first_intersect.seg2_index.saturating_sub(first_intersect.seg1_index) + 2);
        let mut steps2 = 0;

        for idx in (first_intersect.seg1_index + 1)..=first_intersect.seg2_index {
            steps2 += 1;
            if steps2 > MAX_BODY_POINTS {
                break;
            }
            if idx < verts.len() {
                loop2_pts.push(verts[idx]);
            }
        }
        loop2_pts.push(first_intersect.point);

        let ring1 = PolygonRing::new(loop1_pts, ring.is_closed);
        let ring2 = PolygonRing::new(loop2_pts, ring.is_closed);

        let area1 = ring1.signed_area().abs();
        let area2 = ring2.signed_area().abs();

        // Retain the main polygon body (larger area) and trim the self-intersection bowtie tail
        if area1 >= area2 {
            if ring1.vertices.len() >= 3 {
                vec![ring1]
            } else {
                vec![ring.clone()]
            }
        } else {
            if ring2.vertices.len() >= 3 {
                vec![ring2]
            } else {
                vec![ring.clone()]
            }
        }
    }
}
