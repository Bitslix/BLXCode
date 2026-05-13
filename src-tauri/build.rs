use std::fs;
use std::path::{Path, PathBuf};

/// Stage hook scripts into the cargo target dir so the dev binary can
/// resolve them via `BaseDirectory::Resource`. `cargo tauri build`
/// already copies declared resources into the bundle; `cargo tauri dev`
/// does not, which leaves `target/<profile>/hooks/` empty unless we
/// copy them ourselves.
fn stage_dev_resources() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir.parent().expect("workspace root");
    let src_dir = project_root.join("content").join("hooks");
    if !src_dir.is_dir() {
        return;
    }

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    // OUT_DIR = target/<profile>/build/<crate>-<hash>/out  →  three parents up = target/<profile>
    let target_profile_dir = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(out_dir);
    let dest_dir = target_profile_dir.join("hooks");
    let _ = fs::create_dir_all(&dest_dir);

    let entries = match fs::read_dir(&src_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("py") {
            continue;
        }
        let Some(name) = p.file_name() else { continue };
        let dest = dest_dir.join(name);
        let _ = fs::copy(&p, &dest);
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!("cargo:rerun-if-changed={}", src_dir.display());
}

fn main() {
    stage_dev_resources();
    tauri_build::build()
}
