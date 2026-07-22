use vector_core::{PathCommand, Polygon2D, VectorCoreError, VectorDocument, VectorPath};

pub struct EpsWriter;

impl EpsWriter {
    /// Serializes a `Polygon2D` into Adobe PostScript 3.0 EPS format.
    pub fn write_polygon(polygon: &Polygon2D, title: &str) -> Result<String, VectorCoreError> {
        let (min_x, min_y, max_x, max_y) = polygon.bounding_box();
        let llx = min_x.floor() as i64;
        let lly = min_y.floor() as i64;
        let urx = max_x.ceil() as i64;
        let ury = max_y.ceil() as i64;

        let mut eps = String::new();

        // Standard Adobe PostScript 3.0 DSC EPS Header
        eps.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
        eps.push_str(&format!("%%Title: {}\n", title));
        eps.push_str("%%Creator: Vector Geometry Engine\n");
        eps.push_str(&format!("%%BoundingBox: {} {} {} {}\n", llx, lly, urx, ury));
        eps.push_str(&format!("%%HiResBoundingBox: {:.4} {:.4} {:.4} {:.4}\n", min_x, min_y, max_x, max_y));
        eps.push_str("%%EndComments\n\n");

        eps.push_str("gsave\n");
        eps.push_str("newpath\n");

        // Write exterior ring
        if !polygon.exterior.is_empty() {
            eps.push_str(&format!("{:.4} {:.4} moveto\n", polygon.exterior[0].x, polygon.exterior[0].y));
            for pt in &polygon.exterior[1..] {
                eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y));
            }
            eps.push_str("closepath\n");
        }

        // Write interior holes
        for hole in &polygon.interiors {
            if !hole.is_empty() {
                eps.push_str(&format!("{:.4} {:.4} moveto\n", hole[0].x, hole[0].y));
                for pt in &hole[1..] {
                    eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y));
                }
                eps.push_str("closepath\n");
            }
        }

        // Default fill and stroke style
        eps.push_str("0 0 0 setrgbcolor\n");
        eps.push_str("fill\n");
        eps.push_str("grestore\n");
        eps.push_str("showpage\n");
        eps.push_str("%%EOF\n");

        Ok(eps)
    }

    /// Serializes a `VectorDocument` into Adobe PostScript 3.0 EPS format.
    pub fn write_document(doc: &VectorDocument, title: &str) -> Result<String, VectorCoreError> {
        let bbox = doc.view_box.unwrap_or((0.0, 0.0, doc.width.unwrap_or(500.0), doc.height.unwrap_or(500.0)));

        let llx = bbox.0.floor() as i64;
        let lly = bbox.1.floor() as i64;
        let urx = bbox.2.ceil() as i64;
        let ury = bbox.3.ceil() as i64;

        let mut eps = String::new();

        eps.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
        eps.push_str(&format!("%%Title: {}\n", title));
        eps.push_str("%%Creator: Vector Geometry Engine\n");
        eps.push_str(&format!("%%BoundingBox: {} {} {} {}\n", llx, lly, urx, ury));
        eps.push_str(&format!("%%HiResBoundingBox: {:.4} {:.4} {:.4} {:.4}\n", bbox.0, bbox.1, bbox.2, bbox.3));
        eps.push_str("%%EndComments\n\n");

        eps.push_str("gsave\n");

        for path in &doc.paths {
            Self::write_path_commands(&mut eps, path);
        }

        eps.push_str("grestore\n");
        eps.push_str("showpage\n");
        eps.push_str("%%EOF\n");

        Ok(eps)
    }

    fn write_path_commands(eps: &mut String, path: &VectorPath) {
        eps.push_str("newpath\n");
        for cmd in &path.commands {
            match cmd {
                PathCommand::MoveTo(pt) => eps.push_str(&format!("{:.4} {:.4} moveto\n", pt.x, pt.y)),
                PathCommand::LineTo(pt) => eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y)),
                PathCommand::QuadTo { control, end } => eps.push_str(&format!("{:.4} {:.4} {:.4} {:.4} lineto\n", control.x, control.y, end.x, end.y)),
                PathCommand::CubicTo { control1, control2, end } => {
                    eps.push_str(&format!("{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} curveto\n", control1.x, control1.y, control2.x, control2.y, end.x, end.y))
                }
                PathCommand::Close => eps.push_str("closepath\n"),
            }
        }
        eps.push_str("0 0 0 setrgbcolor fill\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::Point2D;

    #[test]
    fn test_eps_writer_dsc_headers() {
        let poly = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(100.0, 0.0),
            Point2D::new(100.0, 100.0), Point2D::new(0.0, 100.0)
        ]);

        let eps_out = EpsWriter::write_polygon(&poly, "Test Square").unwrap();

        assert!(eps_out.starts_with("%!PS-Adobe-3.0 EPSF-3.0\n"));
        assert!(eps_out.contains("%%BoundingBox: 0 0 100 100\n"));
        assert!(eps_out.contains("%%HiResBoundingBox: 0.0004 0.0004 100.0004 100.0004") || eps_out.contains("%%HiResBoundingBox: 0.0000 0.0000 100.0000 100.0000"));
        assert!(eps_out.contains("moveto\n"));
        assert!(eps_out.contains("lineto\n"));
        assert!(eps_out.contains("closepath\n"));
        assert!(eps_out.ends_with("%%EOF\n"));
    }
}
