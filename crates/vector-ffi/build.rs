fn main() {
    println!("cargo:rerun-if-changed=src/inkscape_simplify.cpp");
    println!("cargo:rerun-if-changed=src/inkscape_simplify.h");

    cc::Build::new()
        .cpp(true)
        .file("src/inkscape_simplify.cpp")
        .flag_if_supported("-std=c++17")
        .compile("inkscape_simplify");
}
