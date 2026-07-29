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
        let offset_x = left_w + gap;
        let total_w = offset_x + right_w;

        let mut duo_doc = VectorDocument::new();
        duo_doc.width = Some(total_w);
        duo_doc.height = Some(max_h);
        duo_doc.view_box = Some((0.0, 0.0, total_w, max_h));

        // 1. Add Left side artwork (Design 1) directly
        for path in &left_doc.paths {
            let mut left_path = path.clone();
            left_path.id = Some(format!("left_{}", path.id.as_deref().unwrap_or("path")));
            duo_doc.paths.push(left_path);
        }

        // 2. Add Right side artwork (Design 2) translated by (left_w + gap)
        let offset_x = left_w + gap;
        for path in &right_doc.paths {
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
                    PathCommand::MoveTo(p) => PathCommand::MoveTo(Point2D::new(p.x + offset_x, p.y)),
                    PathCommand::LineTo(p) => PathCommand::LineTo(Point2D::new(p.x + offset_x, p.y)),
                    PathCommand::QuadTo { control, end } => PathCommand::QuadTo {
                        control: Point2D::new(control.x + offset_x, control.y),
                        end: Point2D::new(end.x + offset_x, end.y),
                    },
                    PathCommand::CubicTo { control1, control2, end } => PathCommand::CubicTo {
                        control1: Point2D::new(control1.x + offset_x, control1.y),
                        control2: Point2D::new(control2.x + offset_x, control2.y),
                        end: Point2D::new(end.x + offset_x, end.y),
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
