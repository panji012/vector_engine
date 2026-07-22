//! Computational geometry algorithms crate (Polygon Union, Simplification, Canvas Scaling, Pattern Detection, Pattern Generation, Color & Luminance Variants, Color Grouping, Outset).

pub mod color;
pub mod color_combine;
pub mod luminance;
pub mod outset;
pub mod pattern_detect;
pub mod pattern_gen;
pub mod scale;
pub mod simplify;
pub mod union;

pub use color::ColorVariantEngine;
pub use color_combine::{ColorCombineReport, ColorCombiner, ColorGroupDetail};
pub use luminance::LuminanceEngine;
pub use outset::{GeometryOutset, OutsetReport};
pub use pattern_detect::{PatternDetector, PatternSuitabilityReport};
pub use pattern_gen::PatternGenerator;
pub use scale::CanvasScaler;
pub use simplify::GeometrySimplifier;
pub use union::PolygonUnion;
use vector_core::VectorCoreError;

pub type AlgoResult<T> = Result<T, VectorCoreError>;

/// Skeleton trait for computational geometry operations.
pub trait GeometryAlgorithm<Input, Output> {
    fn execute(&self, input: &Input) -> AlgoResult<Output>;
}
