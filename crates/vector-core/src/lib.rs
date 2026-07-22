pub mod error;
pub mod primitives;
pub mod traits;

pub use error::{CoreResult, VectorCoreError};
pub use primitives::{PathCommand, Point2D, Polygon2D, VectorDocument, VectorPath};
pub use traits::{Geometry2D, Transformable2D};
