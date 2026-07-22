use crate::polygon::PolygonRing;
use crate::topology::{RingRole, TopologyContourNode};

pub struct ContourExtractor;

impl ContourExtractor {
    /// Extracts final valid contours from cleaned offset rings while preserving all micro-details (e.g. eye glints, small dots).
    pub fn extract_contours(
        offset_rings: Vec<PolygonRing>,
        original_nodes: &[TopologyContourNode],
    ) -> (Vec<(PolygonRing, RingRole)>, usize) {
        let mut final_contours = Vec::with_capacity(offset_rings.len());
        let mut discarded_count = 0;

        for (idx, ring) in offset_rings.into_iter().enumerate() {
            if ring.vertices.is_empty() {
                discarded_count += 1;
                continue;
            }

            // Only cull true zero-area / zero-perimeter degenerate lines (< 1e-12)
            let n = ring.vertices.len();
            let area = ring.signed_area().abs();
            let mut perimeter = 0.0;
            if n >= 2 {
                for i in 0..n {
                    let next = (i + 1) % n;
                    perimeter += ring.vertices[i].distance_to(&ring.vertices[next]);
                }
            }

            if n >= 3 && area < 1e-12 && perimeter < 1e-12 {
                discarded_count += 1;
                continue;
            }

            let role = if idx < original_nodes.len() {
                original_nodes[idx].role
            } else {
                RingRole::Exterior
            };

            final_contours.push((ring, role));
        }

        (final_contours, discarded_count)
    }
}
