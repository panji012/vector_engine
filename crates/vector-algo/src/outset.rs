pub use vector_offset::{OutsetReport, VectorOffsetEngine};
use vector_core::{VectorCoreError, VectorDocument};
use crate::color_combine::ColorCombiner;

pub struct GeometryOutset;

impl GeometryOutset {
    /// Outsets all paths in a `VectorDocument` outward by `distance` px using the native Rust processing pipeline.
    ///
    /// Explicitly runs `ColorCombiner::combine_document()` FIRST to group document paths by fill color into compound paths
    /// before executing outset calculations on unified color geometry.
    pub fn outset_document(
        doc: &VectorDocument,
        enable_outset: bool,
        outset_distance: f64,
        enable_simplify: bool,
        simplify_tolerance: f64,
    ) -> Result<(VectorDocument, OutsetReport), VectorCoreError> {
        let (combined_doc, _) = ColorCombiner::combine_document(doc)?;
        VectorOffsetEngine::offset_document(&combined_doc, enable_outset, outset_distance, enable_simplify, simplify_tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{PathCommand, Point2D, VectorPath};

    #[test]
    fn test_vector_algo_outset_reexport() {
        let mut doc = VectorDocument::new();
        let mut path = VectorPath::new(Some("test".to_string()));
        path.fill = Some("#FF0000".to_string());
        path.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(100.0, 0.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(100.0, 100.0)));
        path.add_command(PathCommand::LineTo(Point2D::new(0.0, 100.0)));
        path.add_command(PathCommand::Close);
        doc.paths.push(path);

        let (outset_doc, report) = GeometryOutset::outset_document(&doc, true, 0.5, true, 0.2).unwrap();

        assert_eq!(report.outset_distance, 0.5);
        assert_eq!(report.original_bounds, (0.0, 0.0, 100.0, 100.0));
        assert_eq!(report.new_bounds, (-0.5, -0.5, 100.5, 100.5));
        assert_eq!(outset_doc.paths.len(), 1);
    }
}
