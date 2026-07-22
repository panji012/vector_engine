use crate::polygon::PolygonRing;
use crate::topology::{RingRole, TopologyContourNode};
use vector_core::Point2D;

pub struct PolygonOffsetGenerator;

impl PolygonOffsetGenerator {
    /// Generates raw offset polygon rings for a set of topology nodes.
    ///
    /// Supports positive outset ($d > 0$) and negative inset ($d < 0$).
    /// Exterior rings expand outward for outset (+d) and shrink for inset (-d).
    /// Interior holes shrink inward for outset (-d) and expand for inset (+d).
    pub fn offset_topology(nodes: &[TopologyContourNode], distance: f64) -> Vec<PolygonRing> {
        let mut offset_rings = Vec::with_capacity(nodes.len());

        for node in nodes {
            let effective_dist = match node.role {
                RingRole::Exterior => distance,
                RingRole::Hole => -distance,
            };

            let offset_ring = Self::offset_single_ring(&node.ring, effective_dist);
            offset_rings.push(offset_ring);
        }

        offset_rings
    }

    pub fn offset_single_ring(ring: &PolygonRing, distance: f64) -> PolygonRing {
        let m = ring.vertices.len();
        if m < 3 || distance.abs() <= 1e-9 {
            return ring.clone();
        }

        let area_positive = ring.signed_area() > 0.0;
        let mut offset_pts = Vec::with_capacity(m);
        let max_miter_shift = (distance.abs() * 4.0).max(2.0);

        for i in 0..m {
            let prev_idx = if i == 0 { m - 1 } else { i - 1 };
            let next_idx = (i + 1) % m;

            let p_prev = ring.vertices[prev_idx];
            let p_curr = ring.vertices[i];
            let p_next = ring.vertices[next_idx];

            let e1 = Point2D::new(p_curr.x - p_prev.x, p_curr.y - p_prev.y);
            let e2 = Point2D::new(p_next.x - p_curr.x, p_next.y - p_curr.y);

            let len1 = (e1.x * e1.x + e1.y * e1.y).sqrt().max(1e-9);
            let len2 = (e2.x * e2.x + e2.y * e2.y).sqrt().max(1e-9);

            let u1 = Point2D::new(e1.x / len1, e1.y / len1);
            let u2 = Point2D::new(e2.x / len2, e2.y / len2);

            // Outward normal in Y-down SVG coordinate space
            let n1 = if area_positive { Point2D::new(u1.y, -u1.x) } else { Point2D::new(-u1.y, u1.x) };
            let n2 = if area_positive { Point2D::new(u2.y, -u2.x) } else { Point2D::new(-u2.y, u2.x) };

            let b = Point2D::new(n1.x + n2.x, n1.y + n2.y);
            let dot = (n1.x * n2.x + n1.y * n2.y).clamp(-1.0, 1.0);
            let denom = (1.0 + dot).max(0.15);

            let mut shift_x = (distance * b.x) / denom;
            let mut shift_y = (distance * b.y) / denom;

            // Clamp sharp miter spikes
            shift_x = shift_x.clamp(-max_miter_shift, max_miter_shift);
            shift_y = shift_y.clamp(-max_miter_shift, max_miter_shift);

            offset_pts.push(Point2D::new(p_curr.x + shift_x, p_curr.y + shift_y));
        }

        PolygonRing::new(offset_pts, ring.is_closed)
    }
}
