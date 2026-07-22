use svgtypes::Transform;

fn test() {
    let mut t1: Transform = "translate(10, 20)".parse().unwrap();
    let t2: Transform = "scale(2)".parse().unwrap();
    t1.append(&t2);
    let (x, y) = t1.apply(10.0, 5.0);
}
