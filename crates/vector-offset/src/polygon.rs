use vector_core::Point2D;

/// 2D Line Segment representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment2D {
    pub start: Point2D,
    pub end: Point2D,
}

impl Segment2D {
    pub fn new(start: Point2D, end: Point2D) -> Self {
        Self { start, end }
    }

    pub fn length(&self) -> f64 {
        self.start.distance_to(&self.end)
    }

    pub fn direction(&self) -> Point2D {
        let len = self.length().max(1e-12);
        Point2D::new((self.end.x - self.start.x) / len, (self.end.y - self.start.y) / len)
    }
}

/// Polygon ring representation built from polyline vertices.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonRing {
    pub vertices: Vec<Point2D>,
    pub is_closed: bool,
}

impl PolygonRing {
    pub fn new(vertices: Vec<Point2D>, is_closed: bool) -> Self {
        Self { vertices, is_closed }
    }

    pub fn signed_area(&self) -> f64 {
        let n = self.vertices.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += self.vertices[i].x * self.vertices[j].y - self.vertices[j].x * self.vertices[i].y;
        }
        area / 2.0
    }

    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        if self.vertices.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for v in &self.vertices {
            if v.x < min_x { min_x = v.x; }
            if v.y < min_y { min_y = v.y; }
            if v.x > max_x { max_x = v.x; }
            if v.y > max_y { max_y = v.y; }
        }
        (min_x, min_y, max_x, max_y)
    }

    pub fn segments(&self) -> Vec<Segment2D> {
        let n = self.vertices.len();
        if n < 2 {
            return Vec::new();
        }
        let mut segs = Vec::with_capacity(n);
        let limit = if self.is_closed { n } else { n - 1 };
        for i in 0..limit {
            let next = (i + 1) % n;
            segs.push(Segment2D::new(self.vertices[i], self.vertices[next]));
        }
        segs
    }
}
