/// Build script — ensures Cargo recompiles the crate when static assets change.
/// Without this, rust_embed's procedural macro may serve stale embedded files
/// because Cargo's incremental compilation doesn't track changes to the
/// #[folder] directory contents (only the .rs source file itself).
fn main() {
    println!("cargo::rerun-if-changed=static/admin/");
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/refs/");

    // Build version string with short git commit SHA for --version output.
    let pkg_ver = env!("CARGO_PKG_VERSION");
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    let version = match sha {
        Some(sha) => format!("{pkg_ver} ({sha})"),
        None => pkg_ver.to_string(),
    };

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let version_path = std::path::Path::new(&out_dir).join("version.txt");
    std::fs::write(&version_path, version).expect("failed to write version.txt");
}
