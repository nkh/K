/// Build script — ensures Cargo recompiles the crate when static assets change.
/// Without this, rust_embed's procedural macro may serve stale embedded files
/// because Cargo's incremental compilation doesn't track changes to the
/// #[folder] directory contents (only the .rs source file itself).
fn main() {
    // Track individual static asset files, not just the directory metadata.
    println!("cargo::rerun-if-changed=build.rs");
    if let Ok(entries) = std::fs::read_dir("static/admin") {
        for entry in entries.flatten() {
            println!("cargo::rerun-if-changed={}", entry.path().display());
        }
    }

    // Only track git refs if we're inside a git repository (e.g., not in
    // crates.io tarballs or Nix builds).
    if std::path::Path::new(".git").exists() {
        println!("cargo::rerun-if-changed=.git/HEAD");
        println!("cargo::rerun-if-changed=.git/refs/");
    }

    // Build version string with short git commit SHA for --version output.
    let pkg_ver = env!("CARGO_PKG_VERSION");
    let sha = std::path::Path::new(".git")
        .exists()
        .then(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .flatten();

    let version = match sha {
        Some(sha) => format!("{pkg_ver} ({sha})"),
        None => pkg_ver.to_string(),
    };

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let version_path = std::path::Path::new(&out_dir).join("version.txt");
    std::fs::write(&version_path, version).expect("failed to write version.txt");
}