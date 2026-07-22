fn main() {
    let t: svgtypes::Transform = "translate(10, 20)".parse().unwrap();
    println!("{:?}", t);
}
