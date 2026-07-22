//! Vector geometry serialization and rendering crate (WKT, WKB, GeoJSON, SVG, EPS, JPG Preview).

pub mod eps;
pub mod jpg;
pub mod svg;

pub use eps::EpsWriter;
pub use jpg::JpgPreviewGenerator;
pub use svg::SvgParser;
use vector_core::VectorCoreError;

pub trait VectorParser<T> {
    fn parse_str(&self, raw: &str) -> Result<T, VectorCoreError>;
}

pub trait VectorWriter<T> {
    fn write_str(&self, item: &T) -> Result<String, VectorCoreError>;
}
