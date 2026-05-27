/// Build script — ensures Cargo recompiles the crate when static assets change.
/// Without this, rust_embed's procedural macro may serve stale embedded files
/// because Cargo's incremental compilation doesn't track changes to the
/// #[folder] directory contents (only the .rs source file itself).
fn main() {
    println!("cargo::rerun-if-changed=static/admin/");
}
