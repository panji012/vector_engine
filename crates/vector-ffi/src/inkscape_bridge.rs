use std::ffi::{CStr, CString};
use vector_core::{VectorCoreError, VectorDocument};
use vector_io::SvgParser;

extern "C" {
    fn c_inkscape_simplify_path(
        path_d: *const std::os::raw::c_char,
        tolerance: f64,
        out_buf: *mut std::os::raw::c_char,
        max_len: usize,
    ) -> std::os::raw::c_int;
}

/// Calls native C++ Inkscape Livarot algorithm to simplify an SVG path `d` string.
pub fn inkscape_cpp_simplify_path(path_d: &str, tolerance: f64) -> Result<String, VectorCoreError> {
    let c_path = CString::new(path_d)
        .map_err(|e| VectorCoreError::InvalidGeometry(format!("Nul byte in SVG path: {}", e)))?;

    // Allocate 1MB output buffer for C++ path string
    let max_len = (path_d.len() * 2).max(1024 * 1024);
    let mut out_buf = vec![0u8; max_len];

    let status = unsafe {
        c_inkscape_simplify_path(
            c_path.as_ptr(),
            tolerance,
            out_buf.as_mut_ptr() as *mut std::os::raw::c_char,
            max_len,
        )
    };

    if status != 0 {
        return Err(VectorCoreError::InvalidGeometry(format!(
            "Inkscape C++ simplify path failed with status code {}",
            status
        )));
    }

    let c_str = unsafe { CStr::from_ptr(out_buf.as_ptr() as *const std::os::raw::c_char) };
    let result_str = c_str
        .to_str()
        .map_err(|e| VectorCoreError::InvalidGeometry(format!("Invalid UTF-8 from C++: {}", e)))?;

    Ok(result_str.to_string())
}

/// Simplifies all paths in a `VectorDocument` using C++ Inkscape Livarot simplify engine.
pub fn inkscape_cpp_simplify_document(doc: &VectorDocument, tolerance: f64) -> Result<VectorDocument, VectorCoreError> {
    let mut simplified_doc = doc.clone();

    for path in &mut simplified_doc.paths {
        let d_orig = path.to_svg_path_data();
        if let Ok(d_simp) = inkscape_cpp_simplify_path(&d_orig, tolerance) {
            if let Ok(parsed_doc) = SvgParser::parse_str(&format!("<svg><path d=\"{}\"/></svg>", d_simp)) {
                if let Some(parsed_path) = parsed_doc.paths.first() {
                    path.commands = parsed_path.commands.clone();
                }
            }
        }
    }

    Ok(simplified_doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_cpp_inkscape_simplify() {
        let raw_d = "M 0 0 L 1 0.05 L 2 0.0 L 3 0.05 L 4 0 Z";
        let simp_d = inkscape_cpp_simplify_path(raw_d, 0.2).unwrap();
        assert!(!simp_d.is_empty());
        assert!(simp_d.contains('M'));
    }
}
