fn main() {
  // Do not update files on docsrs
  if std::env::var("DOCS_RS").is_ok() {
    return;
  }
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string());
  if let Ok(bindings) = cbindgen::generate(&crate_dir) {
    bindings.write_to_file(format!("{}/include/crossterm.h", crate_dir));
  }
}
