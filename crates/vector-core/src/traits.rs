/// Base geometry interface abstraction for 2D primitives.
pub trait Geometry2D {
    type CoordinateScalar;

    fn is_valid(&self) -> bool;
    fn bounding_box(&self) -> (Self::CoordinateScalar, Self::CoordinateScalar, Self::CoordinateScalar, Self::CoordinateScalar);
}

/// Transformable trait for applying spatial affine transformations.
pub trait Transformable2D<T> {
    fn translate(&mut self, dx: T, dy: T);
    fn scale(&mut self, sx: T, sy: T);
}
