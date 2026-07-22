import re

with open("crates/vector-offset/src/lib.rs", "r") as f:
    code = f.read()

new_func = """    pub fn offset_document(
        doc: &VectorDocument,
        distance: f64,
    ) -> Result<(VectorDocument, OutsetReport), VectorCoreError> {
        let original_bounds = calculate_document_bounds(doc);

        let input_node_count = AtomicU64::new(0);
        let output_node_count = AtomicU64::new(0);

        let mut final_paths = Vec::new();

        for path in &doc.paths {
            let mut doc_rings: Vec<PolygonRing> = Vec::new();
            
            // 1. Flatten this specific color path's subpaths
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
                final_paths.push(path.clone());
                continue;
            }

            // 2. Build Topology EXCLUSIVELY for this color group!
            // Prevents shapes of different colors from falsely classifying each other as holes
            let topology_nodes = TopologyBuilder::build_topology(doc_rings);

            // 3. Robust Polygon Offset
            let raw_offset_rings = PolygonOffsetGenerator::offset_topology(&topology_nodes, distance);

            // 4. Self-Intersection Detection & Cleanup
            let mut current_contours = Vec::new();

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
                        current_contours.push((clean, role));
                    } else {
                        current_contours.push((original_ring.clone(), role));
                    }
                }
            }

            // 5. Reconstruct SVG Path for this specific color group
            let new_path = PathReconstructor::reconstruct_svg_path(&current_contours, path);
            
            // Telemetry
            for cmd in &new_path.commands {
                match cmd {
                    PathCommand::MoveTo(_) | PathCommand::LineTo(_) => {
                        output_node_count.fetch_add(1, Ordering::Relaxed);
                    }
                    PathCommand::CubicTo { .. } => {
                        output_node_count.fetch_add(3, Ordering::Relaxed);
                    }
                    PathCommand::QuadTo { .. } => {
                        output_node_count.fetch_add(2, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            final_paths.push(new_path);
        }

        let mut new_doc = VectorDocument::new();
        new_doc.paths = final_paths;

        let new_bounds = calculate_document_bounds(&new_doc);
        let report = OutsetReport {
            outset_distance: distance,
            original_bounds,
            new_bounds,
        };
        
        Ok((new_doc, report))
    }"""

# regex to find offset_document entirely up to Ok(...)
pattern = re.compile(r"    pub fn offset_document\(.*?Ok\(\(new_doc, report\)\)\n    \}", re.DOTALL)
new_code = pattern.sub(new_func, code)

with open("crates/vector-offset/src/lib.rs", "w") as f:
    f.write(new_code)
