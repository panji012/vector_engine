use crate::polygon::PolygonRing;
use rayon::prelude::*;
use vector_core::Point2D;

/// Ring role in polygon topology: Exterior boundary or Interior hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingRole {
    Exterior,
    Hole,
}

/// Node in the polygon topology tree representing a contour ring and its relationship.
#[derive(Debug, Clone)]
pub struct TopologyContourNode {
    pub ring: PolygonRing,
    pub role: RingRole,
    pub parent_index: Option<usize>,
    pub children_indices: Vec<usize>,
}

pub struct TopologyBuilder;

impl TopologyBuilder {
    /// Builds topology tree determining ring roles (exterior boundary vs. hole) and parent-child containment.
    ///
    /// Accelerated with spatial bounding box pre-filtering and Rayon parallel evaluation.
    pub fn build_topology(rings: Vec<PolygonRing>) -> Vec<TopologyContourNode> {
        let n = rings.len();
        if n == 0 {
            return Vec::new();
        }

        // Precompute bounding boxes and signed areas for O(1) containment pre-filtering
        let metadata: Vec<((f64, f64, f64, f64), f64)> = rings
            .iter()
            .map(|r| (r.bounding_box(), r.signed_area().abs()))
            .collect();

        let mut nodes: Vec<TopologyContourNode> = rings
            .into_iter()
            .map(|r| TopologyContourNode {
                ring: r,
                role: RingRole::Exterior,
                parent_index: None,
                children_indices: Vec::new(),
            })
            .collect();

        if n == 1 {
            return nodes;
        }

        // Parallel parent containment search across CPU cores with O(1) bounding box rejection
        let parents: Vec<Option<usize>> = (0..n)
            .into_par_iter()
            .map(|i| {
                if nodes[i].ring.vertices.is_empty() {
                    return None;
                }
                let sample_pt = nodes[i].ring.vertices[0];
                let mut best_parent: Option<usize> = None;
                let mut smallest_parent_area = f64::INFINITY;

                for j in 0..n {
                    if i == j {
                        continue;
                    }

                    let ((min_x, min_y, max_x, max_y), area) = metadata[j];

                    // Bounding Box Pre-Filter with tolerance epsilon for edge alignment
                    const EPS: f64 = 1e-4;
                    if sample_pt.x < min_x - EPS || sample_pt.x > max_x + EPS || sample_pt.y < min_y - EPS || sample_pt.y > max_y + EPS {
                        continue;
                    }

                    // Precise ray-casting point-in-polygon test
                    if point_in_polygon(sample_pt, &nodes[j].ring.vertices) {
                        if area < smallest_parent_area {
                            smallest_parent_area = area;
                            best_parent = Some(j);
                        }
                    }
                }

                best_parent
            })
            .collect();

        // Assign parents & determine hole roles
        for i in 0..n {
            if let Some(parent_idx) = parents[i] {
                nodes[i].parent_index = Some(parent_idx);
                let depth = get_depth(&nodes, parent_idx);
                if depth % 2 == 1 {
                    nodes[i].role = RingRole::Hole;
                }
            }
        }

        // Populate children indices
        for i in 0..n {
            if let Some(p) = nodes[i].parent_index {
                nodes[p].children_indices.push(i);
            }
        }

        nodes
    }
}

fn get_depth(nodes: &[TopologyContourNode], idx: usize) -> usize {
    let mut depth = 1;
    let mut curr = idx;
    let mut steps = 0;
    const MAX_DEPTH_STEPS: usize = 2000;

    while let Some(p) = nodes[curr].parent_index {
        depth += 1;
        curr = p;
        steps += 1;
        if steps >= MAX_DEPTH_STEPS {
            break; // Guard against infinite cyclic parent graph loops
        }
    }
    depth
}

fn point_in_polygon(pt: Point2D, poly: &[Point2D]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    let mut steps = 0;
    const MAX_RAY_STEPS: usize = 5000;

    for i in 0..n {
        steps += 1;
        if steps >= MAX_RAY_STEPS {
            break;
        }

        let xi = poly[i].x;
        let yi = poly[i].y;
        let xj = poly[j].x;
        let yj = poly[j].y;

        // Mathematically exact ray casting: check if horizontal ray at pt.y intersects line segment (xi,yi)-(xj,yj)
        if (yi > pt.y) != (yj > pt.y) {
            let denom = yj - yi;
            let x_intersect = (xj - xi) * (pt.y - yi) / denom + xi;
            if pt.x < x_intersect {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}
