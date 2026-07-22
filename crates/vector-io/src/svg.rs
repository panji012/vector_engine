use roxmltree::{Document, Node};
use svgtypes::{PathParser, PathSegment, Transform};
use vector_algo::PolygonUnion;
use vector_core::{CoreResult, PathCommand, Point2D, VectorCoreError, VectorDocument, VectorPath};

#[derive(Debug, Clone, Default)]
pub struct InheritedStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
}

pub struct SvgParser;

impl SvgParser {
    /// Parse an SVG string into a `VectorDocument`.
    pub fn parse_str(svg_content: &str) -> CoreResult<VectorDocument> {
        let clean_content = strip_doctype(svg_content);
        let doc = Document::parse(&clean_content)
            .map_err(|e| VectorCoreError::IoError(format!("XML parse error: {}", e)))?;

        let root = doc.root_element();
        if root.tag_name().name() != "svg" {
            return Err(VectorCoreError::IoError("Root element is not <svg>".to_string()));
        }

        let mut vector_doc = VectorDocument::new();

        if let Some(w) = root.attribute("width") {
            vector_doc.width = parse_length(w);
        }
        if let Some(h) = root.attribute("height") {
            vector_doc.height = parse_length(h);
        }
        if let Some(vb) = root.attribute("viewBox") {
            vector_doc.view_box = parse_viewbox(vb);
        }

        let root_style = resolve_node_style(&root, &InheritedStyle::default());
        let mut group_id_counter = 0;

        Self::traverse_node(&root, &mut vector_doc, &root_style, 0, &mut group_id_counter, Transform::default())?;

        Ok(vector_doc)
    }

    fn apply_transform(t: &Transform, x: f64, y: f64) -> (f64, f64) {
        let nx = t.a * x + t.c * y + t.e;
        let ny = t.b * x + t.d * y + t.f;
        (nx, ny)
    }

    fn append_transform(t1: &Transform, t2: &Transform) -> Transform {
        Transform {
            a: t1.a * t2.a + t1.c * t2.b,
            b: t1.b * t2.a + t1.d * t2.b,
            c: t1.a * t2.c + t1.c * t2.d,
            d: t1.b * t2.c + t1.d * t2.d,
            e: t1.a * t2.e + t1.c * t2.f + t1.e,
            f: t1.b * t2.e + t1.d * t2.f + t1.f,
        }
    }

    /// Helper to parse a single SVG path `d="..."` attribute into a `VectorPath`.
    pub fn parse_d_attribute(d: &str, id: Option<String>, transform: &Transform) -> CoreResult<VectorPath> {
        let mut path = VectorPath::new(id);
        let mut current_pt = Point2D::new(0.0, 0.0);
        let mut subpath_start_pt = Point2D::new(0.0, 0.0);
        let mut is_first_command = true;

        for segment in PathParser::from(d) {
            let segment = segment.map_err(|e| VectorCoreError::IoError(format!("SVG path syntax error: {}", e)))?;
            let apply_tx = |pt: Point2D| -> Point2D {
                let (x, y) = Self::apply_transform(transform, pt.x, pt.y);
                Point2D::new(x, y)
            };

            match segment {
                PathSegment::MoveTo { abs, x, y } => {
                    let pt = if abs || is_first_command { Point2D::new(x, y) } else { Point2D::new(current_pt.x + x, current_pt.y + y) };
                    current_pt = pt;
                    subpath_start_pt = pt;
                    path.add_command(PathCommand::MoveTo(apply_tx(pt)));
                }
                PathSegment::LineTo { abs, x, y } => {
                    let pt = if abs || is_first_command { Point2D::new(x, y) } else { Point2D::new(current_pt.x + x, current_pt.y + y) };
                    current_pt = pt;
                    path.add_command(PathCommand::LineTo(apply_tx(pt)));
                }
                PathSegment::HorizontalLineTo { abs, x } => {
                    let pt = if abs || is_first_command { Point2D::new(x, current_pt.y) } else { Point2D::new(current_pt.x + x, current_pt.y) };
                    current_pt = pt;
                    path.add_command(PathCommand::LineTo(apply_tx(pt)));
                }
                PathSegment::VerticalLineTo { abs, y } => {
                    let pt = if abs || is_first_command { Point2D::new(current_pt.x, y) } else { Point2D::new(current_pt.x, current_pt.y + y) };
                    current_pt = pt;
                    path.add_command(PathCommand::LineTo(apply_tx(pt)));
                }
                PathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
                    let c1 = if abs || is_first_command { Point2D::new(x1, y1) } else { Point2D::new(current_pt.x + x1, current_pt.y + y1) };
                    let c2 = if abs || is_first_command { Point2D::new(x2, y2) } else { Point2D::new(current_pt.x + x2, current_pt.y + y2) };
                    let end = if abs || is_first_command { Point2D::new(x, y) } else { Point2D::new(current_pt.x + x, current_pt.y + y) };
                    current_pt = end;
                    path.add_command(PathCommand::CubicTo { control1: apply_tx(c1), control2: apply_tx(c2), end: apply_tx(end) });
                }
                PathSegment::Quadratic { abs, x1, y1, x, y } => {
                    let c = if abs || is_first_command { Point2D::new(x1, y1) } else { Point2D::new(current_pt.x + x1, current_pt.y + y1) };
                    let end = if abs || is_first_command { Point2D::new(x, y) } else { Point2D::new(current_pt.x + x, current_pt.y + y) };
                    current_pt = end;
                    path.add_command(PathCommand::QuadTo { control: apply_tx(c), end: apply_tx(end) });
                }
                PathSegment::ClosePath { .. } => {
                    path.add_command(PathCommand::Close);
                    current_pt = subpath_start_pt;
                }
                _ => {}
            }
            is_first_command = false;
        }

        Ok(path)
    }

    /// Parse two SVG path `d` strings, compute their Union, and return the output SVG path `d` string.
    pub fn union_svg_d_strings(d1: &str, d2: &str) -> CoreResult<String> {
        let t = Transform::default();
        let vpath1 = Self::parse_d_attribute(d1, None, &t)?;
        let vpath2 = Self::parse_d_attribute(d2, None, &t)?;

        let poly1 = vpath1.to_polygon();
        let poly2 = vpath2.to_polygon();

        let union_polys = PolygonUnion::union(&poly1, &poly2)?;

        let mut d_out = String::new();
        for (i, poly) in union_polys.iter().enumerate() {
            if i > 0 {
                d_out.push(' ');
            }
            d_out.push_str(&poly.to_svg_path_data());
        }
        Ok(d_out)
    }

    fn traverse_node(node: &Node, doc: &mut VectorDocument, current_style: &InheritedStyle, current_group_id: usize, group_id_counter: &mut usize, current_transform: Transform) -> CoreResult<()> {
        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            let mut child_style = resolve_node_style(&child, current_style);
            
            // Calculate child's accumulated transform
            let mut child_transform = current_transform;
            if let Some(transform_str) = child.attribute("transform") {
                if let Ok(parsed_tx) = transform_str.parse::<Transform>() {
                    child_transform = Self::append_transform(&current_transform, &parsed_tx);
                }
            }

            // Approximate uniform scaling for stroke-width
            if let Some(sw) = child_style.stroke_width.as_mut() {
                let sx = child_transform.a.hypot(child_transform.b);
                let sy = child_transform.c.hypot(child_transform.d);
                let scale = (sx + sy) / 2.0;
                *sw *= scale;
            }

            match child.tag_name().name() {
                "path" => {
                    if let Some(mut path) = Self::parse_path_element(&child, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "rect" => {
                    if let Some(mut path) = Self::parse_rect_element(&child, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "circle" => {
                    if let Some(mut path) = Self::parse_circle_element(&child, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "ellipse" => {
                    if let Some(mut path) = Self::parse_ellipse_element(&child, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "line" => {
                    if let Some(mut path) = Self::parse_line_element(&child, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "polygon" | "polyline" => {
                    let is_closed = child.tag_name().name() == "polygon";
                    if let Some(mut path) = Self::parse_poly_element(&child, is_closed, &child_transform)? {
                        apply_style_to_path(&mut path, &child_style);
                        path.group_id = current_group_id;
                        doc.paths.push(path);
                    }
                }
                "g" | "svg" => {
                    *group_id_counter += 1;
                    let next_group_id = *group_id_counter;
                    Self::traverse_node(&child, doc, &child_style, next_group_id, group_id_counter, child_transform)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_path_element(node: &Node, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let d = match node.attribute("d") {
            Some(d) => d,
            None => return Ok(None),
        };

        let path = Self::parse_d_attribute(d, node.attribute("id").map(String::from), transform)?;
        Ok(Some(path))
    }

    fn parse_rect_element(node: &Node, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let x = node.attribute("x").and_then(parse_length).unwrap_or(0.0);
        let y = node.attribute("y").and_then(parse_length).unwrap_or(0.0);
        let w = match node.attribute("width").and_then(parse_length) {
            Some(w) => w,
            None => return Ok(None),
        };
        let h = match node.attribute("height").and_then(parse_length) {
            Some(h) => h,
            None => return Ok(None),
        };

        let apply_tx = |px: f64, py: f64| -> Point2D {
            let (tx, ty) = Self::apply_transform(transform, px, py);
            Point2D::new(tx, ty)
        };

        let mut path = VectorPath::new(node.attribute("id").map(String::from));
        path.add_command(PathCommand::MoveTo(apply_tx(x, y)));
        path.add_command(PathCommand::LineTo(apply_tx(x + w, y)));
        path.add_command(PathCommand::LineTo(apply_tx(x + w, y + h)));
        path.add_command(PathCommand::LineTo(apply_tx(x, y + h)));
        path.add_command(PathCommand::Close);

        Ok(Some(path))
    }

    fn parse_circle_element(node: &Node, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let cx = node.attribute("cx").and_then(parse_length).unwrap_or(0.0);
        let cy = node.attribute("cy").and_then(parse_length).unwrap_or(0.0);
        let r = match node.attribute("r").and_then(parse_length) {
            Some(r) => r,
            None => return Ok(None),
        };

        let m_scale = Transform { a: r, b: 0.0, c: 0.0, d: r, e: 0.0, f: 0.0 };
        let m_translate = Transform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: cx, f: cy };
        let m_shape = Self::append_transform(&m_translate, &m_scale);
        let final_transform = Self::append_transform(transform, &m_shape);

        let k = 0.5522847498307935;
        let mut path = VectorPath::new(node.attribute("id").map(String::from));

        let apply_tx = |px: f64, py: f64| -> Point2D {
            let (tx, ty) = Self::apply_transform(&final_transform, px, py);
            Point2D::new(tx, ty)
        };

        // Generate unit circle bezier curves centered at (0.0, 0.0) with radius 1.0
        // Start at right-center (1.0, 0.0)
        path.add_command(PathCommand::MoveTo(apply_tx(1.0, 0.0)));

        // Top-Right Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(1.0, -k),
            control2: apply_tx(k, -1.0),
            end: apply_tx(0.0, -1.0),
        });

        // Top-Left Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(-k, -1.0),
            control2: apply_tx(-1.0, -k),
            end: apply_tx(-1.0, 0.0),
        });

        // Bottom-Left Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(-1.0, k),
            control2: apply_tx(-k, 1.0),
            end: apply_tx(0.0, 1.0),
        });

        // Bottom-Right Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(k, 1.0),
            control2: apply_tx(1.0, k),
            end: apply_tx(1.0, 0.0),
        });

        path.add_command(PathCommand::Close);

        Ok(Some(path))
    }

    fn parse_ellipse_element(node: &Node, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let cx = node.attribute("cx").and_then(parse_length).unwrap_or(0.0);
        let cy = node.attribute("cy").and_then(parse_length).unwrap_or(0.0);
        let rx = match node.attribute("rx").and_then(parse_length) {
            Some(rx) => rx,
            None => return Ok(None),
        };
        let ry = match node.attribute("ry").and_then(parse_length) {
            Some(ry) => ry,
            None => return Ok(None),
        };

        let m_scale = Transform { a: rx, b: 0.0, c: 0.0, d: ry, e: 0.0, f: 0.0 };
        let m_translate = Transform { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: cx, f: cy };
        let m_shape = Self::append_transform(&m_translate, &m_scale);
        let final_transform = Self::append_transform(transform, &m_shape);

        let k = 0.5522847498307935;
        let mut path = VectorPath::new(node.attribute("id").map(String::from));

        let apply_tx = |px: f64, py: f64| -> Point2D {
            let (tx, ty) = Self::apply_transform(&final_transform, px, py);
            Point2D::new(tx, ty)
        };

        // Generate unit circle bezier curves centered at (0.0, 0.0) with radius 1.0
        // Start at right-center (1.0, 0.0)
        path.add_command(PathCommand::MoveTo(apply_tx(1.0, 0.0)));

        // Top-Right Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(1.0, -k),
            control2: apply_tx(k, -1.0),
            end: apply_tx(0.0, -1.0),
        });

        // Top-Left Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(-k, -1.0),
            control2: apply_tx(-1.0, -k),
            end: apply_tx(-1.0, 0.0),
        });

        // Bottom-Left Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(-1.0, k),
            control2: apply_tx(-k, 1.0),
            end: apply_tx(0.0, 1.0),
        });

        // Bottom-Right Arc
        path.add_command(PathCommand::CubicTo {
            control1: apply_tx(k, 1.0),
            control2: apply_tx(1.0, k),
            end: apply_tx(1.0, 0.0),
        });

        path.add_command(PathCommand::Close);

        Ok(Some(path))
    }

    fn parse_line_element(node: &Node, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let x1 = node.attribute("x1").and_then(parse_length).unwrap_or(0.0);
        let y1 = node.attribute("y1").and_then(parse_length).unwrap_or(0.0);
        let x2 = node.attribute("x2").and_then(parse_length).unwrap_or(0.0);
        let y2 = node.attribute("y2").and_then(parse_length).unwrap_or(0.0);

        let apply_tx = |px: f64, py: f64| -> Point2D {
            let (tx, ty) = Self::apply_transform(transform, px, py);
            Point2D::new(tx, ty)
        };

        let mut path = VectorPath::new(node.attribute("id").map(String::from));
        path.add_command(PathCommand::MoveTo(apply_tx(x1, y1)));
        path.add_command(PathCommand::LineTo(apply_tx(x2, y2)));

        Ok(Some(path))
    }

    fn parse_poly_element(node: &Node, is_closed: bool, transform: &Transform) -> CoreResult<Option<VectorPath>> {
        let points_str = match node.attribute("points") {
            Some(p) => p,
            None => return Ok(None),
        };

        let pts: Vec<Point2D> = points_str
            .split_whitespace()
            .flat_map(|s| s.split(','))
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<f64>>()
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    let (tx, ty) = Self::apply_transform(transform, chunk[0], chunk[1]);
                    Some(Point2D::new(tx, ty))
                } else {
                    None
                }
            })
            .collect();

        if pts.is_empty() {
            return Ok(None);
        }

        let mut path = VectorPath::new(node.attribute("id").map(String::from));
        path.add_command(PathCommand::MoveTo(pts[0]));

        for pt in pts.iter().skip(1) {
            path.add_command(PathCommand::LineTo(*pt));
        }

        if is_closed {
            path.add_command(PathCommand::Close);
        }

        Ok(Some(path))
    }
}

fn apply_style_to_path(path: &mut VectorPath, style: &InheritedStyle) {
    path.fill = style.fill.clone();
    path.stroke = style.stroke.clone();
    path.stroke_width = style.stroke_width;
}

fn resolve_node_style(node: &Node, parent_style: &InheritedStyle) -> InheritedStyle {
    let (style_fill, style_stroke, style_sw) = node
        .attribute("style")
        .map(parse_style_attribute)
        .unwrap_or((None, None, None));

    let fill = match node.attribute("fill") {
        Some("inherit") => parent_style.fill.clone(),
        Some(val) => Some(normalize_color_string(val)),
        None => match style_fill {
            Some(ref val) if val == "inherit" => parent_style.fill.clone(),
            Some(val) => Some(val),
            None => parent_style.fill.clone(),
        },
    };

    let stroke = match node.attribute("stroke") {
        Some("inherit") => parent_style.stroke.clone(),
        Some(val) => Some(normalize_color_string(val)),
        None => match style_stroke {
            Some(ref val) if val == "inherit" => parent_style.stroke.clone(),
            Some(val) => Some(val),
            None => parent_style.stroke.clone(),
        },
    };

    let stroke_width = node
        .attribute("stroke-width")
        .and_then(parse_length)
        .or(style_sw)
        .or(parent_style.stroke_width);

    InheritedStyle {
        fill,
        stroke,
        stroke_width,
    }
}

fn parse_style_attribute(style_str: &str) -> (Option<String>, Option<String>, Option<f64>) {
    let mut fill = None;
    let mut stroke = None;
    let mut stroke_width = None;

    for declaration in style_str.split(';') {
        let mut parts = declaration.splitn(2, ':');
        if let (Some(property), Some(value)) = (parts.next(), parts.next()) {
            let prop = property.trim().to_lowercase();
            let val = value.trim();
            match prop.as_str() {
                "fill" => fill = Some(normalize_color_string(val)),
                "stroke" => stroke = Some(normalize_color_string(val)),
                "stroke-width" => stroke_width = parse_length(val),
                _ => {}
            }
        }
    }
    (fill, stroke, stroke_width)
}

/// Normalizes all common SVG color representations (#RGB, #RRGGBB, rgb(r,g,b), named colors) to standard HEX string or 'none'.
pub fn normalize_color_string(input: &str) -> String {
    let clean = input.trim();
    if clean.is_empty() {
        return "none".to_string();
    }

    let lower = clean.to_lowercase();
    if lower == "none" || lower == "transparent" {
        return "none".to_string();
    }
    if lower == "inherit" {
        return "inherit".to_string();
    }

    // Handle rgb(r, g, b) or rgb(r%, g%, b%)
    if lower.starts_with("rgb(") && lower.ends_with(')') {
        let content = &lower[4..lower.len() - 1];
        let parts: Vec<&str> = content.split(',').collect();
        if parts.len() == 3 {
            let parse_channel = |s: &str| -> u8 {
                let trimmed = s.trim();
                if trimmed.ends_with('%') {
                    let pct: f64 = trimmed[..trimmed.len() - 1].parse().unwrap_or(0.0);
                    (pct / 100.0 * 255.0).round() as u8
                } else {
                    trimmed.parse::<u8>().unwrap_or(0)
                }
            };
            let r = parse_channel(parts[0]);
            let g = parse_channel(parts[1]);
            let b = parse_channel(parts[2]);
            return format!("#{:02X}{:02X}{:02X}", r, g, b);
        }
    }

    // Handle #RGB short hex format (e.g. #00f -> #0000FF)
    if lower.starts_with('#') {
        let hex = &lower[1..];
        if hex.len() == 3 {
            let r = &hex[0..1];
            let g = &hex[1..2];
            let b = &hex[2..3];
            return format!("#{}{}{}{}{}{}", r, r, g, g, b, b).to_uppercase();
        } else if hex.len() == 6 {
            return lower.to_uppercase();
        }
    }

    // Named colors mapping
    match lower.as_str() {
        "black" => "#000000".to_string(),
        "white" => "#FFFFFF".to_string(),
        "red" => "#FF0000".to_string(),
        "green" => "#008000".to_string(),
        "lime" => "#00FF00".to_string(),
        "blue" => "#0000FF".to_string(),
        "yellow" => "#FFFF00".to_string(),
        "cyan" | "aqua" => "#00FFFF".to_string(),
        "magenta" | "fuchsia" => "#FF00FF".to_string(),
        "gray" | "grey" => "#808080".to_string(),
        _ => clean.to_string(),
    }
}

/// Safely strips XML <!DOCTYPE ...> declarations to allow parsing SVGs exported from Adobe Illustrator,
/// Inkscape, and Affinity Designer without triggering roxmltree DTD errors or exposing XXE vulnerabilities.
fn strip_doctype(xml: &str) -> String {
    let mut result = xml.to_string();
    while let Some(start) = result.find("<!DOCTYPE") {
        let bytes = result.as_bytes();
        let mut end = start + 9;
        let mut in_bracket = false;

        while end < bytes.len() {
            match bytes[end] {
                b'[' => in_bracket = true,
                b']' => in_bracket = false,
                b'>' if !in_bracket => {
                    end += 1;
                    break;
                }
                _ => {}
            }
            end += 1;
        }

        result = format!("{}{}", &result[..start], &result[end.min(result.len())..]);
    }
    result
}

fn parse_length(val: &str) -> Option<f64> {
    let clean = val.trim_end_matches(|c: char| c.is_alphabetic() || c == '%');
    clean.parse::<f64>().ok()
}

fn parse_viewbox(val: &str) -> Option<(f64, f64, f64, f64)> {
    let nums: Vec<f64> = val
        .split_whitespace()
        .flat_map(|s| s.split(','))
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    if nums.len() == 4 {
        Some((nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_svg_without_dtd() {
        let svg = r#"<svg width="100" height="100" viewBox="0 0 100 100">
            <path d="M 0 0 L 100 0 L 100 100 Z"/>
        </svg>"#;
        let doc = SvgParser::parse_str(svg).unwrap();
        assert_eq!(doc.width, Some(100.0));
        assert_eq!(doc.paths.len(), 1);
    }

    #[test]
    fn test_parse_svg_with_illustrator_dtd() {
        let svg = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg width="200" height="200" viewBox="0 0 200 200">
    <path id="star" d="M 10 10 L 90 90 Z"/>
</svg>"#;
        let doc = SvgParser::parse_str(svg).unwrap();
        assert_eq!(doc.width, Some(200.0));
        assert_eq!(doc.paths.len(), 1);
        assert_eq!(doc.paths[0].id.as_deref(), Some("star"));
    }

    #[test]
    fn test_parse_svg_primitive_ellipse_and_circle() {
        let svg = r##"<svg width="300" height="300">
            <ellipse id="pupil" cx="50" cy="50" rx="10" ry="15" fill="#030101"/>
            <circle id="eye" cx="150" cy="150" r="20" fill="#FFFFFF"/>
        </svg>"##;
        let doc = SvgParser::parse_str(svg).unwrap();
        assert_eq!(doc.paths.len(), 2);
        assert_eq!(doc.paths[0].id.as_deref(), Some("pupil"));
        assert_eq!(doc.paths[0].fill.as_deref(), Some("#030101"));
        assert_eq!(doc.paths[1].id.as_deref(), Some("eye"));
        assert_eq!(doc.paths[1].fill.as_deref(), Some("#FFFFFF"));
    }

    #[test]
    fn test_svg_color_parsing_styles_and_rgb() {
        let svg = r#"<svg width="200" height="200">
            <g style="fill: rgb(0, 0, 255);">
                <path id="p1" d="M 0 0 L 10 10 Z"/>
                <path id="p2" d="M 10 10 L 20 20 Z" style="fill: #FF0000;"/>
                <rect id="r1" x="0" y="0" width="10" height="10" fill="green"/>
            </g>
        </svg>"#;

        let doc = SvgParser::parse_str(svg).unwrap();
        assert_eq!(doc.paths.len(), 3);
        assert_eq!(doc.paths[0].fill.as_deref(), Some("#0000FF"));
        assert_eq!(doc.paths[1].fill.as_deref(), Some("#FF0000"));
        assert_eq!(doc.paths[2].fill.as_deref(), Some("#008000"));
    }
}
