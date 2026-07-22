fn main() {
    let pts = vec![
        (0.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (0.0, 100.0)
    ];
    let mut area = 0.0;
    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].0 * pts[j].1 - pts[j].0 * pts[i].1;
    }
    area /= 2.0;
    println!("CW Area: {}", area);

    let pts2 = vec![
        (0.0, 0.0),
        (0.0, 100.0),
        (100.0, 100.0),
        (100.0, 0.0)
    ];
    let mut area2 = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area2 += pts2[i].0 * pts2[j].1 - pts2[j].0 * pts2[i].1;
    }
    area2 /= 2.0;
    println!("CCW Area: {}", area2);
}
