use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.ends_with("pc-windows-msvc") {
        return;
    }

    let Some(source) = freetype_dll_path(&target) else {
        return;
    };

    println!("cargo:rerun-if-changed={}", source.display());
    if let Some(library_dir) = source.parent() {
        println!("cargo:rustc-link-search=native={}", library_dir.display());
        println!("cargo:rustc-link-lib=freetype");
    }

    let Ok(out_dir) = env::var("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let Some(profile_dir) = profile_dir_from_out_dir(&out_dir) else {
        return;
    };

    copy_dll(&source, &profile_dir.join("deps"));
    copy_dll(&source, &profile_dir);
}

fn freetype_dll_path(target: &str) -> Option<PathBuf> {
    let arch_dir = if target.starts_with("x86_64-") {
        "x86_64"
    } else if target.starts_with("i686-") {
        "i686"
    } else {
        return None;
    };

    Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../vendor/freetype-msvc")
            .join(arch_dir)
            .join("freetype.dll"),
    )
}

fn profile_dir_from_out_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

fn copy_dll(source: &Path, destination_dir: &Path) {
    if !source.exists() {
        return;
    }

    let _ = fs::create_dir_all(destination_dir);
    let _ = fs::copy(source, destination_dir.join("freetype.dll"));
}
