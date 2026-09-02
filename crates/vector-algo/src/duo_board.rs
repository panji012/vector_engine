use vector_core::{PathCommand, Point2D, VectorCoreError, VectorDocument, VectorPath};

pub struct DuoBoardGenerator;

impl DuoBoardGenerator {
    /// Generates a combined dual-variant board SVG document ("SVG Bad Two"):
    /// - Left side: Artwork in Design 1 (Blue Palette)
    /// - Right side: Artwork in Design 2 (Brown Palette)
    /// Both artworks are positioned side-by-side with a configurable gap.
    pub fn create_duo_board(
        left_doc: &VectorDocument,
        right_doc: &VectorDocument,
        gap: f64,
    ) -> Result<VectorDocument, VectorCoreError> {
        let mut left_w = left_doc.width.unwrap_or(0.0);
        let mut left_h = left_doc.height.unwrap_or(0.0);
        if let Some(vb) = left_doc.view_box {
            left_w = left_w.max(vb.2).max(vb.2 - vb.0);
            left_h = left_h.max(vb.3).max(vb.3 - vb.1);
        }
        for path in &left_doc.paths {
            let (_, _, max_x, max_y) = path.bounding_box();
            if max_x.is_finite() && max_x > left_w {
                left_w = max_x;
            }
            if max_y.is_finite() && max_y > left_h {
                left_h = max_y;
            }
        }
        if left_w <= 0.0 { left_w = 1000.0; }
        if left_h <= 0.0 { left_h = 1000.0; }

        let mut right_w = right_doc.width.unwrap_or(0.0);
        let mut right_h = right_doc.height.unwrap_or(0.0);
        if let Some(vb) = right_doc.view_box {
            right_w = right_w.max(vb.2).max(vb.2 - vb.0);
            right_h = right_h.max(vb.3).max(vb.3 - vb.1);
        }
        for path in &right_doc.paths {
            let (_, _, max_x, max_y) = path.bounding_box();
            if max_x.is_finite() && max_x > right_w {
                right_w = max_x;
            }
            if max_y.is_finite() && max_y > right_h {
                right_h = max_y;
            }
        }
        if right_w <= 0.0 { right_w = 1000.0; }
        if right_h <= 0.0 { right_h = 1000.0; }

        let max_h = left_h.max(right_h);
        let left_y_offset = (max_h - left_h) / 2.0;
        let right_y_offset = (max_h - right_h) / 2.0;

        let offset_x = left_w + gap;
        let total_w = offset_x + right_w;

        let mut duo_doc = VectorDocument::new();
        duo_doc.width = Some(total_w);
        duo_doc.height = Some(max_h);
        duo_doc.view_box = Some((0.0, 0.0, total_w, max_h));

        // 1. Add Left side artwork (Design 1)
        let left_bg_info = crate::background::BackgroundProcessor::find_background(left_doc, left_w, left_h);
        let left_bg_idx = left_bg_info.as_ref().map(|(idx, _)| *idx);
        let left_bg_color = left_bg_info.map(|(_, col)| col).unwrap_or_else(|| "#FFFFFF".to_string());

        let mut left_bg = VectorPath::new(Some("left_full_panel_bg".to_string()));
        left_bg.fill = Some(left_bg_color);
        left_bg.stroke = None;
        left_bg.group_id = usize::MAX;
        left_bg.add_command(PathCommand::MoveTo(Point2D::new(0.0, 0.0)));
        left_bg.add_command(PathCommand::LineTo(Point2D::new(left_w, 0.0)));
        left_bg.add_command(PathCommand::LineTo(Point2D::new(left_w, max_h)));
        left_bg.add_command(PathCommand::LineTo(Point2D::new(0.0, max_h)));
        left_bg.add_command(PathCommand::Close);
        duo_doc.paths.push(left_bg);

        for (idx, path) in left_doc.paths.iter().enumerate() {
            let pid = path.id.as_deref().unwrap_or("").to_lowercase();
            let is_explicit_bg = path.group_id == usize::MAX || pid.contains("background") || pid.contains("canvas_bg");

            if Some(idx) == left_bg_idx || is_explicit_bg {
                continue;
            }

            let mut left_path = VectorPath::new(Some(format!("left_{}", path.id.as_deref().unwrap_or("path"))));
            left_path.fill = path.fill.clone();
            left_path.stroke = path.stroke.clone();
            left_path.stroke_width = path.stroke_width;
            left_path.group_id = path.group_id;

            for cmd in &path.commands {
                let translated_cmd = match cmd {
                    PathCommand::MoveTo(p) => PathCommand::MoveTo(Point2D::new(p.x, p.y + left_y_offset)),
                    PathCommand::LineTo(p) => PathCommand::LineTo(Point2D::new(p.x, p.y + left_y_offset)),
                    PathCommand::QuadTo { control, end } => PathCommand::QuadTo {
                        control: Point2D::new(control.x, control.y + left_y_offset),
                        end: Point2D::new(end.x, end.y + left_y_offset),
                    },
                    PathCommand::CubicTo { control1, control2, end } => PathCommand::CubicTo {
                        control1: Point2D::new(control1.x, control1.y + left_y_offset),
                        control2: Point2D::new(control2.x, control2.y + left_y_offset),
                        end: Point2D::new(end.x, end.y + left_y_offset),
                    },
                    PathCommand::Close => PathCommand::Close,
                };
                left_path.add_command(translated_cmd);
            }
            duo_doc.paths.push(left_path);
        }

        // 2. Add Right side artwork (Design 2)
        let right_bg_info = crate::background::BackgroundProcessor::find_background(right_doc, right_w, right_h);
        let right_bg_idx = right_bg_info.as_ref().map(|(idx, _)| *idx);
        let right_bg_color = right_bg_info.map(|(_, col)| col).unwrap_or_else(|| "#FFFFFF".to_string());

        let mut right_bg = VectorPath::new(Some("right_full_panel_bg".to_string()));
        right_bg.fill = Some(right_bg_color);
        right_bg.stroke = None;
        right_bg.group_id = usize::MAX;
        right_bg.add_command(PathCommand::MoveTo(Point2D::new(offset_x, 0.0)));
        right_bg.add_command(PathCommand::LineTo(Point2D::new(offset_x + right_w, 0.0)));
        right_bg.add_command(PathCommand::LineTo(Point2D::new(offset_x + right_w, max_h)));
        right_bg.add_command(PathCommand::LineTo(Point2D::new(offset_x, max_h)));
        right_bg.add_command(PathCommand::Close);
        duo_doc.paths.push(right_bg);

        for (idx, path) in right_doc.paths.iter().enumerate() {
            let pid = path.id.as_deref().unwrap_or("").to_lowercase();
            let is_explicit_bg = path.group_id == usize::MAX || pid.contains("background") || pid.contains("canvas_bg");

            if Some(idx) == right_bg_idx || is_explicit_bg {
                continue;
            }

            let mut right_path = VectorPath::new(Some(format!(
                "right_{}",
                path.id.as_deref().unwrap_or("path")
            )));
            right_path.fill = path.fill.clone();
            right_path.stroke = path.stroke.clone();
            right_path.stroke_width = path.stroke_width;
            right_path.group_id = path.group_id;

            for cmd in &path.commands {
                let translated_cmd = match cmd {
                    PathCommand::MoveTo(p) => PathCommand::MoveTo(Point2D::new(p.x + offset_x, p.y + right_y_offset)),
                    PathCommand::LineTo(p) => PathCommand::LineTo(Point2D::new(p.x + offset_x, p.y + right_y_offset)),
                    PathCommand::QuadTo { control, end } => PathCommand::QuadTo {
                        control: Point2D::new(control.x + offset_x, control.y + right_y_offset),
                        end: Point2D::new(end.x + offset_x, end.y + right_y_offset),
                    },
                    PathCommand::CubicTo { control1, control2, end } => PathCommand::CubicTo {
                        control1: Point2D::new(control1.x + offset_x, control1.y + right_y_offset),
                        control2: Point2D::new(control2.x + offset_x, control2.y + right_y_offset),
                        end: Point2D::new(end.x + offset_x, end.y + right_y_offset),
                    },
                    PathCommand::Close => PathCommand::Close,
                };
                right_path.add_command(translated_cmd);
            }
            duo_doc.paths.push(right_path);
        }

        Ok(duo_doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_duo_board_side_by_side() {
        let mut left = VectorDocument::new();
        left.view_box = Some((0.0, 0.0, 500.0, 500.0));

        let mut right = VectorDocument::new();
        right.view_box = Some((0.0, 0.0, 500.0, 500.0));

        let duo = DuoBoardGenerator::create_duo_board(&left, &right, 50.0).unwrap();
        assert_eq!(duo.view_box, Some((0.0, 0.0, 1050.0, 500.0)));
    }
}
