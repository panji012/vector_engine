//! Native Rust Outset & Inset Geometry Engine (`vector-offset`).
//!
//! Provides a pure Rust modular 11-stage 2D contour polygon offsetting pipeline inspired by Inkscape's processing workflow:
//! SVG Path -> Adaptive Bézier Flattening -> Polygon Construction -> Topology Construction ->
//! Robust Polygon Offset -> Self-Intersection Detection -> Boolean Resolution -> Hole Preservation ->
//! Polygon Cleanup -> Contour Extraction -> SVG Path Reconstruction.

pub mod boolean;
pub mod cleanup;
pub mod contour;
pub mod flatten;
pub mod intersections;
pub mod offset;
pub mod polygon;
pub mod reconstruct;
pub mod topology;

use boolean::BooleanResolver;
use cleanup::PolygonCleaner;
use flatten::CurveFlattener;
use offset::PolygonOffsetGenerator;
use polygon::PolygonRing;
use reconstruct::PathReconstructor;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use topology::{RingRole, TopologyBuilder};
use vector_core::{PathCommand, VectorCoreError, VectorDocument};

/// Statistics report for Outset / Inset operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutsetReport {
    pub outset_distance: f64,
    pub original_bounds: (f64, f64, f64, f64),
    pub new_bounds: (f64, f64, f64, f64),
}

pub struct VectorOffsetEngine;

impl VectorOffsetEngine {
    /// Executes the 11-stage geometry offset pipeline across document-wide topology with color-group compound path re-combination & RDP node reduction.
    pub fn offset_document(
        doc: &VectorDocument,
        enable_outset: bool,
        outset_distance: f64,
        enable_simplify: bool,
        simplify_tolerance: f64,
    ) -> Result<(VectorDocument, OutsetReport), VectorCoreError> {
        let original_bounds = calculate_document_bounds(doc);

        let effective_distance = if enable_outset { outset_distance } else { 0.0 };

        let input_node_count = AtomicU64::new(0);
        let output_node_count = AtomicU64::new(0);
        let mut total_subpaths = 0;

        let mut color_contour_pairs: Vec<(Option<String>, Vec<(PolygonRing, RingRole)>)> = Vec::new();

        // Process each color group independently to prevent cross-color topology interference
        for path in &doc.paths {
            let color = path.fill.clone();
            let mut doc_rings: Vec<PolygonRing> = Vec::new();

            // Stage 1: Flatten subpaths of the CURRENT color
            let mut subpath_cmds: Vec<Vec<PathCommand>> = Vec::new();
            let mut current_subpath: Vec<PathCommand> = Vec::new();

            for cmd in &path.commands {
                if matches!(cmd, PathCommand::MoveTo(_)) && !current_subpath.is_empty() {
                    subpath_cmds.push(current_subpath);
                    current_subpath = Vec::new();
                }
                current_subpath.push(cmd.clone());
            }
            if !current_subpath.is_empty() {
                subpath_cmds.push(current_subpath);
            }

            for cmds in subpath_cmds {
                let pts = CurveFlattener::flatten_subpath(&cmds, 0.1);
                if !pts.is_empty() {
                    input_node_count.fetch_add(pts.len() as u64, Ordering::Relaxed);
                    doc_rings.push(PolygonRing::new(pts, true));
                }
            }

            if doc_rings.is_empty() {
                continue;
            }

            total_subpaths += doc_rings.len();

            // Stage 2: Topology Tree Construction (Strictly within the SAME color group)
            let topology_nodes = TopologyBuilder::build_topology(doc_rings);

            // Stage 3: Robust Polygon Offset (distance > 0 for outset, < 0 for inset)
            let raw_offset_rings = PolygonOffsetGenerator::offset_topology(&topology_nodes, effective_distance);

            // Stage 4: Self-Intersection Detection, Boolean Resolution & Cleanup with Safety Fallback
            let mut final_rings_for_color = Vec::new();

            for (i, ring) in raw_offset_rings.iter().enumerate() {
                let original_ring = &topology_nodes[i].ring;
                let role = topology_nodes[i].role;

                let resolved_loops = BooleanResolver::resolve_self_intersections(ring);
                let rings_to_use = if resolved_loops.is_empty() {
                    vec![original_ring.clone()]
                } else {
                    resolved_loops
                };

                for loop_ring in rings_to_use {
                    if let Some(clean) = PolygonCleaner::clean_ring(&loop_ring, 1e-12) {
                        final_rings_for_color.push((clean, role));
                    } else {
                        final_rings_for_color.push((original_ring.clone(), role));
                    }
                }
            }

            for (r, _) in &final_rings_for_color {
                output_node_count.fetch_add(r.vertices.len() as u64, Ordering::Relaxed);
            }

            color_contour_pairs.push((color, final_rings_for_color));
        }

        if color_contour_pairs.is_empty() {
            let report = OutsetReport {
                outset_distance: effective_distance,
                original_bounds,
                new_bounds: original_bounds,
            };
            return Ok((doc.clone(), report));
        }

        // Stage 5 & 6: SVG Path Reconstruction with Strict Z-Ordering & Dynamic Simplification
        let final_paths = PathReconstructor::combine_contours_by_color(color_contour_pairs, enable_simplify, simplify_tolerance);

        let mut outset_doc = doc.clone();
        outset_doc.paths = final_paths;

        let new_bounds = calculate_document_bounds(&outset_doc);

        let in_nodes = input_node_count.load(Ordering::Relaxed);
        let out_nodes = output_node_count.load(Ordering::Relaxed);
        let reduction_pct = if in_nodes > 0 {
            (1.0 - (out_nodes as f64 / in_nodes as f64)) * 100.0
        } else {
            0.0
        };

        // Outset pipeline telemetry completed silently

        let report = OutsetReport {
            outset_distance: effective_distance,
            original_bounds,
            new_bounds,
        };

        Ok((outset_doc, report))
    }

    /// Convenience alias for Inset operation with negative distance.
    pub fn inset_document(
        doc: &VectorDocument,
        enable_outset: bool,
        distance: f64,
        enable_simplify: bool,
        simplify_tolerance: f64,
    ) -> Result<(VectorDocument, OutsetReport), VectorCoreError> {
        Self::offset_document(doc, enable_outset, -distance.abs(), enable_simplify, simplify_tolerance)
    }
}

fn calculate_document_bounds(doc: &VectorDocument) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for path in &doc.paths {
        for cmd in &path.commands {
            let pts = match cmd {
                PathCommand::MoveTo(p) | PathCommand::LineTo(p) => vec![*p],
                PathCommand::QuadTo { control, end } => vec![*control, *end],
                PathCommand::CubicTo { control1, control2, end } => vec![*control1, *control2, *end],
                PathCommand::Close => vec![],
            };
            for p in pts {
                if p.x < min_x { min_x = p.x; }
                if p.y < min_y { min_y = p.y; }
                if p.x > max_x { max_x = p.x; }
                if p.y > max_y { max_y = p.y; }
            }
        }
    }

    if min_x.is_infinite() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{Point2D, VectorPath};

    #[test]
    fn test_vector_offset_full_pipeline_outset() {
        let mut doc = VectorDocument::new();
        let mut path = VectorPath::new(Some("square".to_string()));
        path.fill = Some("#FF0000".to_string());
        path.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(100.0, 0.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(100.0, 100.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(0.0, 100.0)));
        path.add_command(PathCommand::Close);

        doc.paths.push(path);

        let (outset_doc, report) = VectorOffsetEngine::offset_document(&doc, true, 0.5, true, 0.2).unwrap();

        assert_eq!(report.outset_distance, 0.5);
        assert_eq!(report.original_bounds, (0.0, 0.0, 100.0, 100.0));
        assert_eq!(report.new_bounds, (-0.5, -0.5, 100.5, 100.5));
        assert_eq!(outset_doc.paths.len(), 1);
        assert_eq!(outset_doc.paths[0].fill.as_deref(), Some("#FF0000"));
    }

    #[test]
    fn test_color_group_recombination_prevents_path_inflation() {
        let mut doc = VectorDocument::new();

        // 3 separate paths with color #0000FF
        for i in 0..3 {
            let mut p = VectorPath::new(Some(format!("blue_{}", i)));
            p.fill = Some("#0000FF".to_string());
            let offset_x = i as f64 * 100.0;
            p.add_command(PathCommand::MoveTo(Point2D::new(offset_x, 0.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x + 50.0, 0.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x + 50.0, 50.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x, 50.0)));
            p.add_command(PathCommand::Close);
            doc.paths.push(p);
        }

        // 2 separate paths with color #FF0000
        for i in 0..2 {
            let mut p = VectorPath::new(Some(format!("red_{}", i)));
            p.fill = Some("#FF0000".to_string());
            let offset_x = i as f64 * 100.0;
            p.add_command(PathCommand::MoveTo(Point2D::new(offset_x, 200.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x + 50.0, 200.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x + 50.0, 250.0)));
            p.add_command(PathCommand::LineTo(Point2D::new(offset_x, 250.0)));
            p.add_command(PathCommand::Close);
            doc.paths.push(p);
        }

        assert_eq!(doc.paths.len(), 5);

        let (outset_doc, _report) = VectorOffsetEngine::offset_document(&doc, true, 0.5, true, 0.2).unwrap();

        // Output MUST contain exactly 2 compound paths (1 per color group)
        assert_eq!(outset_doc.paths.len(), 2, "Output path count must match color group count (2 color groups)");
    }
}
