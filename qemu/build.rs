fn main() {
    if let Some(true) = version_check::is_min_version("1.77.0") {
        println!("cargo:rustc-cfg=has_offset_of");
    }
}
