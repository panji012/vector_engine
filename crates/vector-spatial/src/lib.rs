//! Spatial indexing crate skeleton (R-Tree / BVH).

use vector_core::VectorCoreError;

pub trait SpatialIndex<T> {
    fn insert(&mut self, item: T, bbox: (f64, f64, f64, f64)) -> Result<(), VectorCoreError>;
    fn query_range(&self, bbox: (f64, f64, f64, f64)) -> Vec<&T>;
}
