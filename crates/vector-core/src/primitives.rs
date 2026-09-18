use serde::{Deserialize, Serialize};

/// 2D Point structure
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Point2D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn scale(&mut self, factor: f64) {
        self.x *= factor;
        self.y *= factor;
    }
}

/// 2D Polygon representation (exterior ring + interior holes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Polygon2D {
    pub exterior: Vec<Point2D>,
    pub interiors: Vec<Vec<Point2D>>,
}

impl Polygon2D {
    pub fn new(exterior: Vec<Point2D>) -> Self {
        Self {
            exterior,
            interiors: Vec::new(),
        }
    }

    pub fn with_holes(exterior: Vec<Point2D>, interiors: Vec<Vec<Point2D>>) -> Self {
        Self { exterior, interiors }
    }

    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        if self.exterior.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for pt in &self.exterior {
            if pt.x < min_x { min_x = pt.x; }
            if pt.y < min_y { min_y = pt.y; }
            if pt.x > max_x { max_x = pt.x; }
            if pt.y > max_y { max_y = pt.y; }
        }
        (min_x, min_y, max_x, max_y)
    }

    pub fn scale(&mut self, factor: f64) {
        for pt in &mut self.exterior {
            pt.scale(factor);
        }
        for hole in &mut self.interiors {
            for pt in hole {
                pt.scale(factor);
            }
        }
    }

    pub fn area(&self) -> f64 {
        let ext_area = ring_area(&self.exterior).abs();
        let hole_area: f64 = self.interiors.iter().map(|h| ring_area(h).abs()).sum();
        ext_area - hole_area
    }

    pub fn to_svg_path_data(&self) -> String {
        if self.exterior.is_empty() {
            return String::new();
        }
        let mut d = String::new();
        d.push_str(&format!("M {} {}", self.exterior[0].x, self.exterior[0].y));
        for pt in &self.exterior[1..] {
            d.push_str(&format!(" L {} {}", pt.x, pt.y));
        }
        d.push_str(" Z");

        for hole in &self.interiors {
            if !hole.is_empty() {
                d.push_str(&format!(" M {} {}", hole[0].x, hole[0].y));
                for pt in &hole[1..] {
                    d.push_str(&format!(" L {} {}", pt.x, pt.y));
                }
                d.push_str(" Z");
            }
        }
        d
    }
}

fn ring_area(ring: &[Point2D]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    let n = ring.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += ring[i].x * ring[j].y;
        area -= ring[j].x * ring[i].y;
    }
    area / 2.0
}

/// Vector path command enum representing path segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PathCommand {
    MoveTo(Point2D),
    LineTo(Point2D),
    QuadTo { control: Point2D, end: Point2D },
    CubicTo { control1: Point2D, control2: Point2D, end: Point2D },
    Close,
}

/// Vector path containing a sequence of commands and optional metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VectorPath {
    pub id: Option<String>,
    pub commands: Vec<PathCommand>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub fill_rule: Option<String>,
    pub group_id: usize,
    #[serde(default)]
    pub badge_id: Option<usize>,
}

impl VectorPath {
    pub fn new(id: Option<String>) -> Self {
        Self {
            id,
            commands: Vec::new(),
            fill: None,
            stroke: None,
            stroke_width: None,
            fill_rule: None,
            group_id: 0,
            badge_id: None,
        }
    }

    pub fn add_command(&mut self, cmd: PathCommand) {
        self.commands.push(cmd);
    }

    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        if self.commands.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        let mut update_pt = |pt: &Point2D| {
            if pt.x < min_x { min_x = pt.x; }
            if pt.y < min_y { min_y = pt.y; }
            if pt.x > max_x { max_x = pt.x; }
            if pt.y > max_y { max_y = pt.y; }
        };

        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(pt) | PathCommand::LineTo(pt) => update_pt(pt),
                PathCommand::QuadTo { control, end } => {
                    update_pt(control);
                    update_pt(end);
                }
                PathCommand::CubicTo { control1, control2, end } => {
                    update_pt(control1);
                    update_pt(control2);
                    update_pt(end);
                }
                PathCommand::Close => {}
            }
        }

        if min_x.is_infinite() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (min_x, min_y, max_x, max_y)
        }
    }

    /// Converts path commands directly to SVG path data string (`d="..."`), preserving exact coordinates,
    /// curves (`CubicTo`, `QuadTo`), sub-paths (`MoveTo`), holes, and winding order.
    pub fn to_svg_path_data(&self) -> String {
        let mut d = String::new();
        for (i, cmd) in self.commands.iter().enumerate() {
            if i > 0 {
                d.push(' ');
            }
            match cmd {
                PathCommand::MoveTo(pt) => d.push_str(&format!("M {} {}", pt.x, pt.y)),
                PathCommand::LineTo(pt) => d.push_str(&format!("L {} {}", pt.x, pt.y)),
                PathCommand::QuadTo { control, end } => {
                    d.push_str(&format!("Q {} {}, {} {}", control.x, control.y, end.x, end.y))
                }
                PathCommand::CubicTo { control1, control2, end } => {
                    d.push_str(&format!(
                        "C {} {}, {} {}, {} {}",
                        control1.x, control1.y, control2.x, control2.y, end.x, end.y
                    ))
                }
                PathCommand::Close => d.push('Z'),
            }
        }
        d
    }

    pub fn to_polygon(&self) -> Polygon2D {
        let mut exterior = Vec::new();
        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(pt) | PathCommand::LineTo(pt) => exterior.push(*pt),
                PathCommand::QuadTo { end, .. } => exterior.push(*end),
                PathCommand::CubicTo { end, .. } => exterior.push(*end),
                PathCommand::Close => {}
            }
        }
        Polygon2D::new(exterior)
    }
}

/// Parsed vector graphics document containing paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VectorDocument {
    pub width: Option<f64>,
    pub height: Option<f64>,
    #[serde(rename = "viewBox")]
    pub view_box: Option<(f64, f64, f64, f64)>,
    pub paths: Vec<VectorPath>,
}

impl VectorDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Serializes the `VectorDocument` into a valid SVG XML string.
    pub fn to_svg_string(&self) -> String {
        let w_attr = self.width.map(|w| format!(" width=\"{}\"", w)).unwrap_or_default();
        let h_attr = self.height.map(|h| format!(" height=\"{}\"", h)).unwrap_or_default();
        let vb_attr = self.view_box.map(|vb| format!(" viewBox=\"{} {} {} {}\"", vb.0, vb.1, vb.2, vb.3)).unwrap_or_default();

        let mut svg = format!(
            "<svg{}{}{} xmlns=\"http://www.w3.org/2000/svg\">\n",
            w_attr, h_attr, vb_attr
        );

        let mut ordered_paths: Vec<&VectorPath> = Vec::with_capacity(self.paths.len());
        for path in &self.paths {
            if path.badge_id.is_none() {
                ordered_paths.push(path);
            }
        }
        let mut unique_badge_ids = Vec::new();
        for path in &self.paths {
            if let Some(b_id) = path.badge_id {
                if !unique_badge_ids.contains(&b_id) {
                    unique_badge_ids.push(b_id);
                }
            }
        }
        for &b_id in &unique_badge_ids {
            for path in &self.paths {
                if path.badge_id == Some(b_id) {
                    ordered_paths.push(path);
                }
            }
        }

        let mut current_badge_group: Option<usize> = None;

        for path in ordered_paths {
            if path.badge_id != current_badge_group {
                if current_badge_group.is_some() {
                    svg.push_str("  </g>\n");
                }
                current_badge_group = path.badge_id;
                if let Some(b_id) = current_badge_group {
                    svg.push_str(&format!("  <g id=\"badge_{}\">\n", b_id));
                }
            }

            let indent = if current_badge_group.is_some() { "    " } else { "  " };
            let id_attr = path.id.as_deref().map(|id| format!(" id=\"{}\"", id)).unwrap_or_default();
            let fill_attr = match path.fill.as_deref() {
                Some(f) => format!(" fill=\"{}\"", f),
                None => " fill=\"none\"".to_string(),
            };
            let stroke_attr = path.stroke.as_deref().map(|s| format!(" stroke=\"{}\"", s)).unwrap_or_default();
            let sw_attr = path.stroke_width.map(|sw| format!(" stroke-width=\"{}\"", sw)).unwrap_or_default();
            let fr_attr = match path.fill_rule.as_deref() {
                Some(fr) => format!(" fill-rule=\"{}\"", fr),
                None => " fill-rule=\"evenodd\"".to_string(),
            };

            let d_data = path.to_svg_path_data();

            svg.push_str(&format!(
                "{}<path{} d=\"{}\"{}{}{}{}/>\n",
                indent, id_attr, d_data, fill_attr, stroke_attr, sw_attr, fr_attr
            ));
        }

        if current_badge_group.is_some() {
            svg.push_str("  </g>\n");
        }

        svg.push_str("</svg>\n");
        svg
    }
}
