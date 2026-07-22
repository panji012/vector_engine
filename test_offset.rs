use std::f64;

#[derive(Debug, Clone, Copy)]
struct Point2D { x: f64, y: f64 }

fn main() {
    let pts = vec![
        Point2D { x: 0.0, y: 0.0 },
        Point2D { x: 10.0, y: 0.0 },
        Point2D { x: 10.0, y: 10.0 },
        Point2D { x: 0.0, y: 10.0 },
    ];
    let distance = 0.5;
    let m = pts.len();
    let mut sum = 0.0;
    for i in 0..m {
        let p1 = pts[i];
        let p2 = pts[(i + 1) % m];
        sum += p1.x * p2.y - p2.x * p1.y;
    }
    let area_positive = (sum * 0.5) > 0.0;
    println!("Area positive: {}", area_positive);

    for i in 0..m {
        let prev_idx = if i == 0 { m - 1 } else { i - 1 };
        let next_idx = (i + 1) % m;
        let p_prev = pts[prev_idx];
        let p_curr = pts[i];
        let p_next = pts[next_idx];

        let e1 = Point2D { x: p_curr.x - p_prev.x, y: p_curr.y - p_prev.y };
        let e2 = Point2D { x: p_next.x - p_curr.x, y: p_next.y - p_curr.y };
        
        let len1 = (e1.x * e1.x + e1.y * e1.y).sqrt().max(1e-9);
        let len2 = (e2.x * e2.x + e2.y * e2.y).sqrt().max(1e-9);
        let u1 = Point2D { x: e1.x / len1, y: e1.y / len1 };
        let u2 = Point2D { x: e2.x / len2, y: e2.y / len2 };

        let n1 = if area_positive { Point2D { x: u1.y, y: -u1.x } } else { Point2D { x: -u1.y, y: u1.x } };
        let n2 = if area_positive { Point2D { x: u2.y, y: -u2.x } } else { Point2D { x: -u2.y, y: u2.x } };

        let b = Point2D { x: n1.x + n2.x, y: n1.y + n2.y };
        let dot = (n1.x * n2.x + n1.y * n2.y).clamp(-1.0, 1.0);
        let denom = (1.0 + dot).max(0.15);

        let shift_x = (distance * b.x) / denom;
        let shift_y = (distance * b.y) / denom;

        println!("p_curr: ({}, {}) -> shift: ({}, {}) -> new: ({}, {})", p_curr.x, p_curr.y, shift_x, shift_y, p_curr.x + shift_x, p_curr.y + shift_y);
    }
}
