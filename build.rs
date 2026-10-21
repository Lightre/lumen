fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("ProductName", "Lumen")
            .set("FileDescription", "Lumen Photo Viewer")
            .set("InternalName", "lumen")
            .set("OriginalFilename", "lumen.exe")
            .set("LegalCopyright", "Copyright (C) 2026 Lightre")
            .compile()
            .expect("Windows application resources must compile");
    }
}
