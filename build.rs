fn main() {
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let bindings = cbindgen::generate(&crate_dir).expect("Failed to generate bindings");
  bindings.write_to_file(format!("{}/include/libcrossterm.h", crate_dir));
}
