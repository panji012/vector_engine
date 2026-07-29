use vector_core::{PathCommand, Point2D, Polygon2D, VectorCoreError, VectorDocument};

/// Canvas Scaling operator.
pub struct CanvasScaler;

impl CanvasScaler {
    /// Scales a `VectorDocument` so that its total pixel area (W * H) is between 19,000,000 px and 22,000,000 px.
    ///
    /// Target nominal pixel area = 20,500,000 px.
    pub fn scale_document_to_pixel_area(
        doc: &VectorDocument,
        min_area: f64,
        max_area: f64,
    ) -> Result<(VectorDocument, f64), VectorCoreError> {
        let (current_w, current_h) = match doc.view_box {
            Some(vb) => ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs()),
            None => (doc.width.unwrap_or(1000.0), doc.height.unwrap_or(1000.0)),
        };

        if current_w <= 0.0 || current_h <= 0.0 {
            return Err(VectorCoreError::InvalidGeometry("Invalid document dimensions for scaling".to_string()));
        }

        let current_area = current_w * current_h;

        // If already within 19M - 22M px range, do not scale
        if current_area >= min_area && current_area <= max_area {
            return Ok((doc.clone(), 1.0));
        }

        let target_area = (min_area + max_area) / 2.0; // Nominal 20,500,000 px
        let scale_factor = (target_area / current_area).sqrt();

        let new_w = current_w * scale_factor;
        let new_h = current_h * scale_factor;

        let mut scaled_doc = doc.clone();
        scaled_doc.width = Some(new_w);
        scaled_doc.height = Some(new_h);

        if let Some(vb) = doc.view_box {
            scaled_doc.view_box = Some((vb.0 * scale_factor, vb.1 * scale_factor, vb.2 * scale_factor, vb.3 * scale_factor));
        } else {
            scaled_doc.view_box = Some((0.0, 0.0, new_w, new_h));
        }

        // Scale all path point coordinates
        for path in &mut scaled_doc.paths {
            for cmd in &mut path.commands {
                match cmd {
                    PathCommand::MoveTo(p) | PathCommand::LineTo(p) => {
                        *p = Point2D::new(p.x * scale_factor, p.y * scale_factor);
                    }
                    PathCommand::QuadTo { control, end } => {
                        *control = Point2D::new(control.x * scale_factor, control.y * scale_factor);
                        *end = Point2D::new(end.x * scale_factor, end.y * scale_factor);
                    }
                    PathCommand::CubicTo { control1, control2, end } => {
                        *control1 = Point2D::new(control1.x * scale_factor, control1.y * scale_factor);
                        *control2 = Point2D::new(control2.x * scale_factor, control2.y * scale_factor);
                        *end = Point2D::new(end.x * scale_factor, end.y * scale_factor);
                    }
                    PathCommand::Close => {}
                }
            }
        }

        Ok((scaled_doc, scale_factor))
    }

    /// Scales a `VectorDocument` using an explicit uniform scale factor.
    pub fn scale_document_by_factor(
        doc: &VectorDocument,
        scale_factor: f64,
    ) -> Result<VectorDocument, VectorCoreError> {
        if scale_factor <= 0.0 {
            return Err(VectorCoreError::InvalidGeometry("Scale factor must be positive".to_string()));
        }

        let (current_w, current_h) = match doc.view_box {
            Some(vb) => ((vb.2 - vb.0).abs(), (vb.3 - vb.1).abs()),
            None => (doc.width.unwrap_or(1000.0), doc.height.unwrap_or(1000.0)),
        };

        let new_w = current_w * scale_factor;
        let new_h = current_h * scale_factor;

        let mut scaled_doc = doc.clone();
        scaled_doc.width = Some(new_w);
        scaled_doc.height = Some(new_h);

        if let Some(vb) = doc.view_box {
            scaled_doc.view_box = Some((vb.0 * scale_factor, vb.1 * scale_factor, vb.2 * scale_factor, vb.3 * scale_factor));
        } else {
            scaled_doc.view_box = Some((0.0, 0.0, new_w, new_h));
        }

        for path in &mut scaled_doc.paths {
            for cmd in &mut path.commands {
                match cmd {
                    PathCommand::MoveTo(p) | PathCommand::LineTo(p) => {
                        *p = Point2D::new(p.x * scale_factor, p.y * scale_factor);
                    }
                    PathCommand::QuadTo { control, end } => {
                        *control = Point2D::new(control.x * scale_factor, control.y * scale_factor);
                        *end = Point2D::new(end.x * scale_factor, end.y * scale_factor);
                    }
                    PathCommand::CubicTo { control1, control2, end } => {
                        *control1 = Point2D::new(control1.x * scale_factor, control1.y * scale_factor);
                        *control2 = Point2D::new(control2.x * scale_factor, control2.y * scale_factor);
                        *end = Point2D::new(end.x * scale_factor, end.y * scale_factor);
                    }
                    PathCommand::Close => {}
                }
            }
        }

        Ok(scaled_doc)
    }

    /// Scales a polygon up to `target_pixels` ONLY if its max dimension is below `target_pixels`.
    pub fn scale_polygon_below_target(poly: &Polygon2D, target_pixels: f64) -> Result<(Polygon2D, f64), VectorCoreError> {
        let (min_x, min_y, max_x, max_y) = poly.bounding_box();
        let current_w = (max_x - min_x).abs();
        let current_h = (max_y - min_y).abs();
        let current_dim = current_w.max(current_h);

        if current_dim <= 0.0 || current_dim >= target_pixels {
            return Ok((poly.clone(), 1.0));
        }

        let scale_factor = target_pixels / current_dim;
        let mut scaled_poly = poly.clone();
        scaled_poly.scale(scale_factor);

        Ok((scaled_poly, scale_factor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_document_to_19M_22M_px() {
        let mut doc = VectorDocument::new();
        doc.view_box = Some((0.0, 0.0, 1000.0, 1000.0)); // 1,000,000 px area

        let (scaled, factor) = CanvasScaler::scale_document_to_pixel_area(&doc, 19_000_000.0, 22_000_000.0).unwrap();
        let (w, h) = match scaled.view_box {
            Some(vb) => (vb.2 - vb.0, vb.3 - vb.1),
            None => (scaled.width.unwrap(), scaled.height.unwrap()),
        };

        let new_area = w * h;
        assert!(new_area >= 19_000_000.0 && new_area <= 22_000_000.0);
        assert!((new_area - 20_500_000.0).abs() < 10.0);
        assert!((factor - (20.5f64).sqrt()).abs() < 1e-4);
    }
}

