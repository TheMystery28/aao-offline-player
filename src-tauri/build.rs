use std::path::{Path, PathBuf};

fn main() {
    tauri_build::build();
    generate_engine_embed();
}

/// Generate a Rust file that embeds all static engine files via `include_bytes!`.
///
/// On Android, Tauri's `app.fs().read()` corrupts binary data when reading from
/// APK assets. To avoid this, we embed the engine files directly in the binary
/// at compile time and extract them to the writable directory on first launch.
/// This also generates engine_files.txt for backward compatibility.
fn generate_engine_embed() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let engine_dir = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .join("engine");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    if !engine_dir.exists() {
        // Engine dir might not exist in CI — write empty files
        std::fs::write(out_path.join("engine_files.txt"), "").unwrap();
        std::fs::write(
            out_path.join("engine_embed.rs"),
            "pub static EMBEDDED_ENGINE_FILES: &[(&str, &[u8])] = &[];\n",
        )
        .unwrap();
        return;
    }

    let mut files = Vec::new();
    collect_engine_files(&engine_dir, &engine_dir, &mut files);
    files.sort(); // deterministic output

    // Write text manifest (kept for reference/debugging)
    std::fs::write(out_path.join("engine_files.txt"), files.join("\n")).unwrap();

    // Generate Rust file with include_bytes! for each engine file.
    // Uses absolute paths from the engine directory so include_bytes! works
    // regardless of the OUT_DIR location.
    let engine_dir_str = engine_dir.to_str().unwrap().replace('\\', "/");
    let mut rust_code = String::new();
    rust_code.push_str("pub static EMBEDDED_ENGINE_FILES: &[(&str, &[u8])] = &[\n");
    for file in &files {
        rust_code.push_str(&format!(
            "    (\"{}\", include_bytes!(\"{}/{}\")),\n",
            file, engine_dir_str, file
        ));
    }
    rust_code.push_str("];\n");

    std::fs::write(out_path.join("engine_embed.rs"), rust_code).unwrap();

    // Rerun if engine static files change
    println!("cargo:rerun-if-changed=../engine/player.html");
    println!("cargo:rerun-if-changed=../engine/bridge.js");
    println!("cargo:rerun-if-changed=../engine/localstorage_bridge.html");
    println!("cargo:rerun-if-changed=../engine/Javascript");
    println!("cargo:rerun-if-changed=../engine/CSS");
    println!("cargo:rerun-if-changed=../engine/Languages");
    println!("cargo:rerun-if-changed=../engine/img");
}

/// Recursively collect engine file paths, skipping runtime data directories.
fn collect_engine_files(base: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(base).unwrap();
        let name = relative.to_str().unwrap().replace('\\', "/");

        // Skip runtime data directories, dev-only directories, and config files
        if name == "case"
            || name.starts_with("case/")
            || name == "defaults"
            || name.starts_with("defaults/")
            || name == "assets"
            || name.starts_with("assets/")
            || name == "tests"
            || name.starts_with("tests/")
            || name == "config.json"
        {
            continue;
        }

        if path.is_file() {
            files.push(name);
        } else if path.is_dir() {
            collect_engine_files(base, &path, files);
        }
    }
}
