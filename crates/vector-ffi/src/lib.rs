use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use vector_algo::{CanvasScaler, ColorCombiner, ColorVariantEngine, GeometryOutset, GeometrySimplifier, LuminanceEngine, PatternDetector, PatternGenerator, PolygonUnion};
use vector_core::Polygon2D;
use vector_io::{EpsWriter, JpgPreviewGenerator, SvgParser};

pub mod inkscape_bridge;
pub mod logging;

pub use inkscape_bridge::{inkscape_cpp_simplify_document, inkscape_cpp_simplify_path};

/// Native C++ Inkscape Livarot Path Simplify function accessible via PyO3 FFI.
#[pyfunction]
pub fn inkscape_cpp_simplify_svg(svg_content: &str, tolerance: f64) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let simplified_doc = inkscape_cpp_simplify_document(&doc, tolerance)
        .map_err(|e| PyValueError::new_err(format!("Inkscape C++ simplify failed: {}", e)))?;

    Ok(simplified_doc.to_svg_string())
}


/// Reads an SVG file from `input_path`, combines paths by color group, applies outset by `distance`,
/// and writes the processed SVG to `output_path`.
///
/// Catches Rust panics and errors safely, raising them as catchable Python exceptions so one failing file does not crash Python.
#[pyfunction]
pub fn process_svg_outset(
    input_path: &str,
    output_path: &str,
    enable_outset: bool,
    outset_distance: f64,
    enable_simplify: bool,
    simplify_tolerance: f64,
) -> PyResult<String> {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        let content = std::fs::read_to_string(input_path)
            .map_err(|e| format!("Failed to read input SVG '{}': {}", input_path, e))?;

        let mut doc = SvgParser::parse_str(&content)
            .map_err(|e| format!("SVG parse error: {}", e))?;

        // 0. Detect and normalize background
        vector_algo::BackgroundProcessor::normalize_background(&mut doc);

        let (combined_doc, _) = ColorCombiner::combine_document(&doc)
            .map_err(|e| format!("Color Group Combine failed: {}", e))?;

        let (outset_doc, _) = GeometryOutset::outset_document(&combined_doc, enable_outset, outset_distance, enable_simplify, simplify_tolerance)
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

    Ok(tiled_doc.to_svg_string())
}

/// Generates a seamless tiled pattern SVG document with configurable style ("slanted_left", "alternating", "grid"), slant ratio, wraparound, and shape filter option.
#[pyfunction]
pub fn generate_pattern_styled_json(
    svg_content: &str,
    grid_x: u32,
    grid_y: u32,
    style: Option<&str>,
    detect_suitable_only: Option<bool>,
    slant_offset_ratio: Option<f64>,
    wraparound_seamless: Option<bool>,
) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let pattern_style = style.unwrap_or("slanted_left");
    let suitable_only = detect_suitable_only.unwrap_or(true);
    let slant_ratio = slant_offset_ratio.unwrap_or(-0.5);
    let wraparound = wraparound_seamless.unwrap_or(true);

    let tiled_doc = PatternGenerator::generate_pattern_full_config(
        &doc,
        grid_x,
        grid_y,
        pattern_style,
        slant_ratio,
        wraparound,
        suitable_only,
    )
    .map_err(|e| PyValueError::new_err(format!("Pattern Generation failed: {}", e)))?;

    Ok(tiled_doc.to_svg_string())
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

    Ok(variant_doc.to_svg_string())
}

/// Adjusts the luminance / brightness of colors in an SVG document using a configurable factor.
#[pyfunction]
pub fn adjust_luminance_json(svg_content: &str, factor: f64) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let adjusted_doc = LuminanceEngine::adjust_document_luminance(&doc, factor)
        .map_err(|e| PyValueError::new_err(format!("Luminance Engine failed: {}", e)))?;

    Ok(adjusted_doc.to_svg_string())
}

/// Groups SVG paths by fill color and combines paths of identical color. Returns (combined_doc_json, report_json).
#[pyfunction]
pub fn combine_color_groups_json(svg_content: &str) -> PyResult<(String, String)> {
    let mut doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    vector_algo::BackgroundProcessor::normalize_background(&mut doc);

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
pub fn apply_outset_json(
    svg_content: &str,
    enable_outset: bool,
    outset_distance: f64,
    enable_simplify: bool,
    simplify_tolerance: f64,
) -> PyResult<(String, String)> {
    let mut doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    vector_algo::BackgroundProcessor::normalize_background(&mut doc);

    let (combined_doc, _) = ColorCombiner::combine_document(&doc)
        .map_err(|e| PyValueError::new_err(format!("Color Group Combine failed: {}", e)))?;

    let (outset_doc, report) = GeometryOutset::outset_document(&combined_doc, enable_outset, outset_distance, enable_simplify, simplify_tolerance)
        .map_err(|e| PyValueError::new_err(format!("Outset operation failed: {}", e)))?;

    let doc_svg = outset_doc.to_svg_string();
    let report_json = serde_json::to_string(&report)
        .map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))?;

    Ok((doc_svg, report_json))
}

/// Applies Design 1 Variant (Blue palette, 80% bg luminance).
#[pyfunction]
pub fn apply_design1_variant_json(svg_content: &str) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let variant_doc = vector_algo::LuminanceEngine::apply_design1_variant(&doc)
        .map_err(|e| PyValueError::new_err(format!("Design 1 variant failed: {}", e)))?;

    Ok(variant_doc.to_svg_string())
}

/// Applies Design 2 Variant (Brown palette, 97.5% bg luminance).
#[pyfunction]
pub fn apply_design2_variant_json(svg_content: &str) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let variant_doc = vector_algo::LuminanceEngine::apply_design2_variant(&doc)
        .map_err(|e| PyValueError::new_err(format!("Design 2 variant failed: {}", e)))?;

    Ok(variant_doc.to_svg_string())
}

/// Scales SVG document total pixel area (W * H) to target range [min_area, max_area] (19M - 22M px).
#[pyfunction]
pub fn scale_document_area_json(svg_content: &str, min_area: f64, max_area: f64) -> PyResult<(String, f64)> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let (scaled_doc, factor) = CanvasScaler::scale_document_to_pixel_area(&doc, min_area, max_area)
        .map_err(|e| PyValueError::new_err(format!("Document scaling failed: {}", e)))?;

    Ok((scaled_doc.to_svg_string(), factor))
}

/// Scales SVG document using an explicit uniform scale factor.
#[pyfunction]
pub fn scale_document_by_factor_json(svg_content: &str, factor: f64) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let scaled_doc = CanvasScaler::scale_document_by_factor(&doc, factor)
        .map_err(|e| PyValueError::new_err(format!("Document scaling failed: {}", e)))?;

    Ok(scaled_doc.to_svg_string())
}

/// Creates a side-by-side Dual Variant board SVG ("SVG Bad Two"): Left = Design 1, Right = Design 2.
#[pyfunction]
pub fn create_duo_board_json(left_svg_content: &str, right_svg_content: &str, gap: f64) -> PyResult<String> {
    let left_doc = SvgParser::parse_str(left_svg_content)
        .map_err(|e| PyValueError::new_err(format!("Left SVG parse error: {}", e)))?;
    let right_doc = SvgParser::parse_str(right_svg_content)
        .map_err(|e| PyValueError::new_err(format!("Right SVG parse error: {}", e)))?;

    let duo_doc = vector_algo::DuoBoardGenerator::create_duo_board(&left_doc, &right_doc, gap)
        .map_err(|e| PyValueError::new_err(format!("Duo Board creation failed: {}", e)))?;

    Ok(duo_doc.to_svg_string())
}


/// Applies Silhouette Variant with configurable lightness threshold, line_art flag, custom bg/fg colors, invert option, and background removal detection.
#[pyfunction]
pub fn apply_silhouette_variant_json(
    svg_content: &str,
    threshold: Option<f64>,
    line_art: Option<bool>,
    bg_color: Option<&str>,
    fg_color: Option<&str>,
    invert: Option<bool>,
    bg_already_removed: Option<bool>,
) -> PyResult<String> {
    let doc = SvgParser::parse_str(svg_content)
        .map_err(|e| PyValueError::new_err(format!("SVG parse error: {}", e)))?;

    let t = threshold.unwrap_or(0.50);
    let is_line_art = line_art.unwrap_or(false);
    let is_invert = invert.unwrap_or(false);
    let bg_removed = bg_already_removed.unwrap_or(false);
    let variant_doc = vector_algo::LuminanceEngine::apply_silhouette_variant_full(
        &doc,
        t,
        is_line_art,
        bg_color,
        fg_color,
        is_invert,
        bg_removed,
    )
    .map_err(|e| PyValueError::new_err(format!("Silhouette variant failed: {}", e)))?;

    Ok(variant_doc.to_svg_string())
}

/// PyO3 C-extension module entry point for vector_engine._native.
#[pymodule]
fn _native(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(logging::init_tracing, m)?)?;
    m.add_function(wrap_pyfunction!(inkscape_cpp_simplify_svg, m)?)?;
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
    m.add_function(wrap_pyfunction!(generate_pattern_styled_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_color_palette_json, m)?)?;
    m.add_function(wrap_pyfunction!(adjust_luminance_json, m)?)?;
    m.add_function(wrap_pyfunction!(combine_color_groups_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_outset_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_design1_variant_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_design2_variant_json, m)?)?;
    m.add_function(wrap_pyfunction!(apply_silhouette_variant_json, m)?)?;
    m.add_function(wrap_pyfunction!(scale_document_area_json, m)?)?;
    m.add_function(wrap_pyfunction!(scale_document_by_factor_json, m)?)?;
    m.add_function(wrap_pyfunction!(create_duo_board_json, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

