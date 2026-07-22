use svgtypes::Transform;
use vector_core::Point2D;

pub fn apply_tx(pt: Point2D, tx: &Transform) -> Point2D {
    let (x, y) = tx.apply(pt.x, pt.y);
    Point2D::new(x, y)
}
