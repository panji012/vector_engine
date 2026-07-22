use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use vector_algo::{CanvasScaler, ColorCombiner, ColorVariantEngine, GeometryOutset, GeometrySimplifier, LuminanceEngine, PatternDetector, PatternGenerator, PolygonUnion};
use vector_core::Polygon2D;
use vector_io::{EpsWriter, JpgPreviewGenerator, SvgParser};

pub mod logging;

/// Reads an SVG file from `input_path`, combines paths by color group, applies outset by `distance`,
/// and writes the processed SVG to `output_path`.
///
/// Catches Rust panics and errors safely, raising them as catchable Python exceptions so one failing file does not crash Python.
#[pyfunction]
pub fn process_svg_outset(input_path: &str, output_path: &str, distance: f64) -> PyResult<String> {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        let content = std::fs::read_to_string(input_path)
            .map_err(|e| format!("Failed to read input SVG '{}': {}", input_path, e))?;

        let doc = SvgParser::parse_str(&content)
            .map_err(|e| format!("SVG parse error: {}", e))?;

        let (combined_doc, _) = ColorCombiner::combine_document(&doc)
            .map_err(|e| format!("Color Group Combine failed: {}", e))?;

        let (outset_doc, _) = GeometryOutset::outset_document(&combined_doc, distance)
            .map_err(|e| format!("Outset operation failed: {}", e))?;

        let output_svg_string = outset_doc.to_svg_string();

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        std::fs::write(output_path, &output_svg_string)
            .map_err(|e| format!("Failed to write output SVG '{}': {}", output_path, e))?;

        Ok(output_svg_string)
    });

    match result {
        Ok(Ok(res)) => Ok(res),
        Ok(Err(err_msg)) => Err(PyValueError::new_err(err_msg)),
        Err(_) => Err(PyValueError::new_err(format!(
            "Panic trapped during SVG outset processing for '{}'", input_path
        ))),
    }
}

/// Parses an SVG string and returns the document structure as a JSON string.
#[pyfunction]
pub fn parse_svg_string(svg_content: &str) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    serde_json::to_string(&doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Computes the Union of two JSON-encoded 2D polygons.
#[pyfunction]
pub fn union_polygons_json(poly1_json: &str, poly2_json: &str) -> PyResult<String> {
    let poly1: Polygon2D = serde_json::from_str(poly1_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon 1 JSON: {}", e)))?;
    let poly2: Polygon2D = serde_json::from_str(poly2_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon 2 JSON: {}", e)))?;

    let result = PolygonUnion::union(&poly1, &poly2)
        .map_err(|e| PyValueError::new_err(format!("Union operation failed: {}", e)))?;

    serde_json::to_string(&result)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Computes the Union of two SVG path `d` strings and returns the combined SVG path `d` string.
#[pyfunction]
pub fn union_svg_path_strings(d1: &str, d2: &str) -> PyResult<String> {
    SvgParser::union_svg_d_strings(d1, d2)
        .map_err(|e| PyValueError::new_err(format!("SVG Path Union failed: {}", e)))
}

/// Simplifies a JSON-encoded 2D polygon using RDP algorithm with configurable tolerance.
#[pyfunction]
pub fn simplify_polygon_json(poly_json: &str, tolerance: f64) -> PyResult<String> {
    let poly: Polygon2D = serde_json::from_str(poly_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon JSON: {}", e)))?;

    let simplified = GeometrySimplifier::simplify_polygon(&poly, tolerance)
        .map_err(|e| PyValueError::new_err(format!("Simplification failed: {}", e)))?;

    serde_json::to_string(&simplified)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Scales a JSON-encoded 2D polygon up to target_pixels ONLY if its max dimension is below target_pixels.
#[pyfunction]
pub fn scale_canvas_json(poly_json: &str, target_pixels: f64) -> PyResult<(String, f64)> {
    let poly: Polygon2D = serde_json::from_str(poly_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon JSON: {}", e)))?;

    let (scaled, factor) = CanvasScaler::scale_polygon_below_target(&poly, target_pixels)
        .map_err(|e| PyValueError::new_err(format!("Canvas scaling failed: {}", e)))?;

    let json_str = serde_json::to_string(&scaled)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;

    Ok((json_str, factor))
}

/// Exports a JSON-encoded 2D polygon into Adobe PostScript 3.0 EPS format.
#[pyfunction]
pub fn export_eps_json(poly_json: &str, title: Option<&str>) -> PyResult<String> {
    let poly: Polygon2D = serde_json::from_str(poly_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon JSON: {}", e)))?;

    let title_str = title.unwrap_or("Vector Engine Export");
    EpsWriter::write_polygon(&poly, title_str)
        .map_err(|e| PyValueError::new_err(format!("EPS Export failed: {}", e)))
}

/// Renders a JSON-encoded 2D polygon into a JPEG preview image at configurable resolution.
#[pyfunction]
pub fn render_jpg_preview_json<'py>(
    py: Python<'py>,
    poly_json: &str,
    width: u32,
    height: u32,
    quality: Option<u8>,
) -> PyResult<&'py PyBytes> {
    let poly: Polygon2D = serde_json::from_str(poly_json)
        .map_err(|e| PyValueError::new_err(format!("Failed to parse polygon JSON: {}", e)))?;

    let q = quality.unwrap_or(85);
    let jpg_bytes = JpgPreviewGenerator::render_polygon_jpg(&poly, width, height, q)
        .map_err(|e| PyValueError::new_err(format!("JPG rendering failed: {}", e)))?;

    Ok(PyBytes::new(py, &jpg_bytes))
}

/// Analyzes an SVG string for seamless pattern suitability and returns JSON report.
#[pyfunction]
pub fn analyze_pattern_suitability_json(svg_content: &str) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let report = PatternDetector::analyze_document(&doc)
        .map_err(|e| PyValueError::new_err(format!("Pattern analysis error: {}", e)))?;

    serde_json::to_string(&report)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Generates a seamless tiled pattern SVG document ONLY if PatternDetector approves. Preserves original colors.
#[pyfunction]
pub fn generate_pattern_svg_json(svg_content: &str, grid_x: u32, grid_y: u32) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let tiled_doc = PatternGenerator::generate_pattern(&doc, grid_x, grid_y)
        .map_err(|e| PyValueError::new_err(format!("Pattern Generation failed: {}", e)))?;

    serde_json::to_string(&tiled_doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Applies a custom HEX color palette (or preset name) to an SVG document.
#[pyfunction]
pub fn apply_color_palette_json(svg_content: &str, palette: Vec<String>) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let resolved_palette = if palette.len() == 1 && !palette[0].starts_with('#') {
        ColorVariantEngine::resolve_preset(&palette[0])
    } else {
        palette
    };

    let variant_doc = ColorVariantEngine::apply_palette(&doc, &resolved_palette)
        .map_err(|e| PyValueError::new_err(format!("Color Variant Engine failed: {}", e)))?;

    serde_json::to_string(&variant_doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Adjusts the luminance / brightness of colors in an SVG document using a configurable factor.
#[pyfunction]
pub fn adjust_luminance_json(svg_content: &str, factor: f64) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let adjusted_doc = LuminanceEngine::adjust_document_luminance(&doc, factor)
        .map_err(|e| PyValueError::new_err(format!("Luminance Engine failed: {}", e)))?;

    serde_json::to_string(&adjusted_doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

/// Groups SVG paths by fill color and combines paths of identical color. Returns (combined_doc_json, report_json).
#[pyfunction]
pub fn combine_color_groups_json(svg_content: &str) -> PyResult<(String, String)> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let (combined_doc, report) = ColorCombiner::combine_document(&doc)
        .map_err(|e| PyValueError::new_err(format!("Color Group Combine failed: {}", e)))?;

    let doc_json = serde_json::to_string(&combined_doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;
    let report_json = serde_json::to_string(&report)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;

    Ok((doc_json, report_json))
}

/// Applies an outward offset (Outset) to SVG document geometry by configurable distance (default 0.5 px).
/// Runs ColorCombiner FIRST before outset calculation.
#[pyfunction]
pub fn apply_outset_json(svg_content: &str, distance: Option<f64>) -> PyResult<(String, String)> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let dist = distance.unwrap_or(0.5);
    let (combined_doc, _) = ColorCombiner::combine_document(&doc)
        .map_err(|e| PyValueError::new_err(format!("Color Group Combine failed: {}", e)))?;

    let (outset_doc, report) = GeometryOutset::outset_document(&combined_doc, dist)
        .map_err(|e| PyValueError::new_err(format!("Outset operation failed: {}", e)))?;

    let doc_json = serde_json::to_string(&outset_doc)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;
    let report_json = serde_json::to_string(&report)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;

    Ok((doc_json, report_json))
}

/// PyO3 C-extension module entry point for vector_engine._native.
#[pymodule]
fn _native(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(logging::init_tracing, m)?)?;
    m.add_function(wrap_pyfunction!(process_svg_outset, m)?)?;
    m.add_function(wrap_pyfunction!(parse_svg_string, m)?)?;
    m.add_function(wrap_pyfunction!(union_polygons_json, m)?)?;
    m.add_function(wrap_pyfunction!(union_svg_path_strings, m)?)?;
    m.add_function(wrap_pyfunction!(simplify_polygon_json, m)?)?;
    m.add_function(wrap_pyfunction!(scale_canvas_json, m)?)?;
    m.add_function(wrap_pyfunction!(export_eps_json, m)?)?;
    m.add_function(wrap_pyfunction!(render_jpg_preview_json, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_pattern_suitability_json, m)?)?;
    m.add_function(wrap_pyfunction!(generate_pattern_svg_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_color_palette_json, m)?)?;
    m.add_function(wrap_pyfunction!(adjust_luminance_json, m)?)?;
    m.add_function(wrap_pyfunction!(combine_color_groups_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_outset_json, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
