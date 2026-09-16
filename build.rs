use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=pdfium.dll");
    let out_dir = match env::var("OUT_DIR") {
        Ok(val) => val,
        Err(_) => return,
    };

    // OUT_DIR is typically target/{profile}/build/{pkg}/out
    // We want target/{profile}
    let target_dir = match Path::new(&out_dir).ancestors().nth(3) {
        Some(dir) => dir,
        None => return,
    };

    let dll_src = Path::new("pdfium.dll");
    if dll_src.exists() {
        let dest = target_dir.join("pdfium.dll");
        let _ = fs::copy(dll_src, dest);
    }
}
