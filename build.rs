fn main() {
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string());
  if let Ok(bindings) = cbindgen::generate(&crate_dir) {
    bindings.write_to_file(format!("{}/include/crossterm.h", crate_dir));
  }
}
