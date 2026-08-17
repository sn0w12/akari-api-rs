use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/og/css");

    let css_dir = Path::new("assets/og/css");
    let mut css_content = String::new();

    let mut entries: Vec<_> = fs::read_dir(css_dir)
        .expect("Failed to read CSS directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("css") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    entries.sort();

    for path in entries {
        let content = fs::read_to_string(&path).expect(&format!("Failed to read {:?}", path));
        css_content.push_str(&content);
        css_content.push('\n');
    }

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("css_embed.rs");
    let code = format!("pub const CSS: &str = r#\"{}\"#;", css_content);
    fs::write(&dest_path, code).expect("Failed to write css_embed.rs");
}
