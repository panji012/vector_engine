use vector_core::{PathCommand, Polygon2D, VectorCoreError, VectorDocument, VectorPath};

pub struct EpsWriter;

impl EpsWriter {
    /// Serializes a `Polygon2D` into Adobe PostScript 3.0 EPS format.
    pub fn write_polygon(polygon: &Polygon2D, title: &str) -> Result<String, VectorCoreError> {
        let (min_x, min_y, max_x, max_y) = polygon.bounding_box();
        let llx = min_x.floor() as i64;
        let lly = min_y.floor() as i64;
        let urx = max_x.ceil() as i64;
        let ury = max_y.ceil() as i64;

        let mut eps = String::new();

        // Standard Adobe PostScript 3.0 DSC EPS Header
        eps.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
        eps.push_str(&format!("%%Title: {}\n", title));
        eps.push_str("%%Creator: Vector Geometry Engine\n");
        eps.push_str(&format!("%%BoundingBox: {} {} {} {}\n", llx, lly, urx, ury));
        eps.push_str(&format!("%%HiResBoundingBox: {:.4} {:.4} {:.4} {:.4}\n", min_x, min_y, max_x, max_y));
        eps.push_str("%%EndComments\n\n");

        eps.push_str("gsave\n");
        eps.push_str("newpath\n");

        // Write exterior ring
        if !polygon.exterior.is_empty() {
            eps.push_str(&format!("{:.4} {:.4} moveto\n", polygon.exterior[0].x, polygon.exterior[0].y));
            for pt in &polygon.exterior[1..] {
                eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y));
            }
            eps.push_str("closepath\n");
        }

        // Write interior holes
        for hole in &polygon.interiors {
            if !hole.is_empty() {
                eps.push_str(&format!("{:.4} {:.4} moveto\n", hole[0].x, hole[0].y));
                for pt in &hole[1..] {
                    eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y));
                }
                eps.push_str("closepath\n");
            }
        }

        // Default fill and stroke style
        eps.push_str("0 0 0 setrgbcolor\n");
        eps.push_str("fill\n");
        eps.push_str("grestore\n");
        eps.push_str("showpage\n");
        eps.push_str("%%EOF\n");

        Ok(eps)
    }

    /// Serializes a `VectorDocument` into Adobe PostScript 3.0 EPS format.
    pub fn write_document(doc: &VectorDocument, title: &str) -> Result<String, VectorCoreError> {
        let bbox = doc.view_box.unwrap_or((0.0, 0.0, doc.width.unwrap_or(500.0), doc.height.unwrap_or(500.0)));

        // Scale PostScript points (72 DPI) so when opened in 96 DPI software (Inkscape), the canvas matches exact SVG screen pixels.
        let dpi_scale = 72.0 / 96.0; // 0.75
        let (min_x_ps, min_y_ps, w_ps, h_ps) = (bbox.0 * dpi_scale, bbox.1 * dpi_scale, bbox.2 * dpi_scale, bbox.3 * dpi_scale);

        let llx = min_x_ps.floor() as i64;
        let lly = min_y_ps.floor() as i64;
        let urx = w_ps.ceil() as i64;
        let ury = h_ps.ceil() as i64;

        let mut eps = String::new();

        eps.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
        eps.push_str(&format!("%%Title: {}\n", title));
        eps.push_str("%%Creator: Vector Geometry Engine\n");
        eps.push_str(&format!("%%BoundingBox: {} {} {} {}\n", llx, lly, urx, ury));
        eps.push_str(&format!("%%HiResBoundingBox: {:.4} {:.4} {:.4} {:.4}\n", min_x_ps, min_y_ps, w_ps, h_ps));
        eps.push_str("%%EndComments\n\n");

        eps.push_str("/u { gsave } def\n");
        eps.push_str("/U { grestore } def\n\n");

        eps.push_str("gsave\n");
        eps.push_str(&format!("0 {:.4} translate 0.75 -0.75 scale\n", h_ps));

        let mut current_badge: Option<usize> = None;

        let has_any_badge = doc.paths.iter().any(|p| p.badge_id.is_some());
        let mut sorted_paths: Vec<&VectorPath> = Vec::with_capacity(doc.paths.len());

        if has_any_badge {
            for p in &doc.paths {
                if p.badge_id.is_none() {
                    sorted_paths.push(p);
                }
            }
            let mut badge_ids: Vec<usize> = Vec::new();
            for p in &doc.paths {
                if let Some(b_id) = p.badge_id {
                    if !badge_ids.contains(&b_id) {
                        badge_ids.push(b_id);
                    }
                }
            }
            badge_ids.sort();

            for b_id in badge_ids {
                for p in &doc.paths {
                    if p.badge_id == Some(b_id) {
                        sorted_paths.push(p);
                    }
                }
            }
        } else {
            for p in &doc.paths {
                sorted_paths.push(p);
            }
        }

        for path in sorted_paths {
            if path.badge_id != current_badge {
                if current_badge.is_some() {
                    eps.push_str("%AI5_EndLayer\n");
                }
                current_badge = path.badge_id;
                if let Some(b_id) = current_badge {
                    eps.push_str("%AI5_BeginLayer\n");
                    eps.push_str(&format!("%1 1 1 1 0 0 0 0 0 0 0 (badge_{}) Adobe_Layer_Group\n", b_id));
                    eps.push_str("%0 0 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 Lb\n");
                    eps.push_str(&format!("%(badge_{}) Ln\n", b_id));
                }
            }
            Self::write_path_commands(&mut eps, path);
        }

        if current_badge.is_some() {
            eps.push_str("%AI5_EndLayer\n");
        }

        eps.push_str("grestore\n");
        eps.push_str("showpage\n");
        eps.push_str("%%EOF\n");

        Ok(eps)
    }

    fn write_path_commands(eps: &mut String, path: &VectorPath) {
        if path.commands.is_empty() {
            return;
        }

        let mut current_pt = vector_core::Point2D::new(0.0, 0.0);
        let mut subpath_start_pt = vector_core::Point2D::new(0.0, 0.0);

        eps.push_str("newpath\n");
        for cmd in &path.commands {
            match cmd {
                PathCommand::MoveTo(pt) => {
                    current_pt = *pt;
                    subpath_start_pt = *pt;
                    eps.push_str(&format!("{:.4} {:.4} moveto\n", pt.x, pt.y));
                }
                PathCommand::LineTo(pt) => {
                    current_pt = *pt;
                    eps.push_str(&format!("{:.4} {:.4} lineto\n", pt.x, pt.y));
                }
                PathCommand::QuadTo { control, end } => {
                    // Convert Quadratic Bézier (P0, Pc, Pe) to Cubic Bézier (P0, C1, C2, Pe)
                    let c1_x = (current_pt.x + 2.0 * control.x) / 3.0;
                    let c1_y = (current_pt.y + 2.0 * control.y) / 3.0;
                    let c2_x = (end.x + 2.0 * control.x) / 3.0;
                    let c2_y = (end.y + 2.0 * control.y) / 3.0;
                    eps.push_str(&format!(
                        "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} curveto\n",
                        c1_x, c1_y, c2_x, c2_y, end.x, end.y
                    ));
                    current_pt = *end;
                }
                PathCommand::CubicTo { control1, control2, end } => {
                    eps.push_str(&format!(
                        "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} curveto\n",
                        control1.x, control1.y, control2.x, control2.y, end.x, end.y
                    ));
                    current_pt = *end;
                }
                PathCommand::Close => {
                    eps.push_str("closepath\n");
                    current_pt = subpath_start_pt;
                }
            }
        }

        let fill_rgb = path.fill.as_deref().and_then(parse_color_rgb);
        let stroke_rgb = path.stroke.as_deref().and_then(parse_color_rgb);

        if fill_rgb.is_some() && stroke_rgb.is_some() {
            if let Some((r, g, b)) = fill_rgb {
                eps.push_str("gsave\n");
                eps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
                if path.fill_rule.as_deref() == Some("evenodd") {
                    eps.push_str("eofill\n");
                } else {
                    eps.push_str("fill\n");
                }
                eps.push_str("grestore\n");
            }
            if let Some((r, g, b)) = stroke_rgb {
                let width = path.stroke_width.unwrap_or(1.0);
                eps.push_str(&format!("{:.4} setlinewidth\n", width));
                eps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
                eps.push_str("stroke\n");
            }
        } else if let Some((r, g, b)) = fill_rgb {
            eps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
            if path.fill_rule.as_deref() == Some("evenodd") {
                eps.push_str("eofill\n");
            } else {
                eps.push_str("fill\n");
            }
        } else if let Some((r, g, b)) = stroke_rgb {
            let width = path.stroke_width.unwrap_or(1.0);
            eps.push_str(&format!("{:.4} setlinewidth\n", width));
            eps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
            eps.push_str("stroke\n");
        }
    }
}

fn parse_color_rgb(color_str: &str) -> Option<(f64, f64, f64)> {
    let clean = color_str.trim();
    if clean.is_empty() || clean.eq_ignore_ascii_case("none") {
        return None;
    }
    let hex = clean.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
        Some((r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
    } else {
        Some((0.0, 0.0, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::Point2D;

    #[test]
    fn test_eps_writer_dsc_headers() {
        let poly = Polygon2D::new(vec![
            Point2D::new(0.0, 0.0), Point2D::new(100.0, 0.0),
            Point2D::new(100.0, 100.0), Point2D::new(0.0, 100.0)
        ]);

        let eps_out = EpsWriter::write_polygon(&poly, "Test Square").unwrap();

        assert!(eps_out.starts_with("%!PS-Adobe-3.0 EPSF-3.0\n"));
        assert!(eps_out.contains("%%BoundingBox: 0 0 100 100\n"));
        assert!(eps_out.contains("%%HiResBoundingBox: 0.0004 0.0004 100.0004 100.0004") || eps_out.contains("%%HiResBoundingBox: 0.0000 0.0000 100.0000 100.0000"));
        assert!(eps_out.contains("moveto\n"));
        assert!(eps_out.contains("lineto\n"));
        assert!(eps_out.contains("closepath\n"));
        assert!(eps_out.ends_with("%%EOF\n"));
    }
}
