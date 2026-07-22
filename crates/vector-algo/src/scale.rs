use vector_core::{Polygon2D, VectorCoreError};

/// Canvas Scaling operator.
pub struct CanvasScaler;

impl CanvasScaler {
    /// Scales a polygon up to `target_pixels` ONLY if its max dimension is below `target_pixels`.
    ///
    /// Returns the scaled polygon and the applied scale factor.
    pub fn scale_polygon_below_target(poly: &Polygon2D, target_pixels: f64) -> Result<(Polygon2D, f64), VectorCoreError> {
        let (min_x, min_y, max_x, max_y) = poly.bounding_box();
        let current_w = (max_x - min_x).abs();
        let current_h = (max_y - min_y).abs();
        let current_dim = current_w.max(current_h);

        if current_dim <= 0.0 || current_dim >= target_pixels {
            // Already at or above target, leave untouched
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
    use vector_core::Point2D;

    #[test]
    fn test_scale_only_below_target() {
        // 100x100 square
        let small_poly = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(100.0, 0.0),
            Point2D::new(100.0, 100.0), Point2D::new(0.0, 100.0)
        ]);

        // Target = 1024 pixels. Small poly should scale up 10.24x
        let (scaled, factor) = CanvasScaler::scale_polygon_below_target(&small_poly, 1024.0).unwrap();
        assert!((factor - 10.24).abs() < 1e-5);
        let (_, _, max_x, max_y) = scaled.bounding_box();
        assert!((max_x - 1024.0).abs() < 1e-5);
        assert!((max_y - 1024.0).abs() < 1e-5);

        // Target = 50 pixels. Small poly is ALREADY above target (100 >= 50), so no scaling occurs (factor = 1.0)
        let (untouched, factor_no_op) = CanvasScaler::scale_polygon_below_target(&small_poly, 50.0).unwrap();
        assert_eq!(factor_no_op, 1.0);
        assert_eq!(untouched, small_poly);
    }
}
