fn main() {
    // The SPA is embedded from ../frontend/build; make sure the folder exists so the backend
    // also builds without the frontend (it then serves a placeholder page).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../frontend/build");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    println!("cargo:rerun-if-changed=../frontend/build");
    println!("cargo:rerun-if-changed=migrations");
}
