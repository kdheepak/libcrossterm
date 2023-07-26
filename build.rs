use std::{env, fs::File, io::Read, path::Path};

fn create_colors() {
  // Do not update files on docsrs
  if std::env::var("DOCS_RS").is_ok() {
    return;
  }
  let out_dir = env::var("OUT_DIR").unwrap();
  let dest_path = Path::new(&out_dir).join("colors.rs");
  let mut file = File::open("./data/colors.json").expect("Could not open colors.json");
  let mut contents = String::new();
  file.read_to_string(&mut contents).expect("Could not read colors.json");
  let data: serde_json::Value = serde_json::from_str(&contents).expect("Could not parse JSON");
  std::fs::write(dest_path, format!("pub static COLORS: &str = r##\"{}\"##;", data)).unwrap();
}

fn create_crossterm_header() {
  // Do not update files on docsrs
  if std::env::var("DOCS_RS").is_ok() {
    return;
  }
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or(".".to_string());
  if let Ok(bindings) = cbindgen::generate(&crate_dir) {
    bindings.write_to_file(format!("{}/include/crossterm.h", crate_dir));
  }
}

fn main() {
  create_colors();
  create_crossterm_header();
}
