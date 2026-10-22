use std::path::{Path, PathBuf};
use std::process::Command;

fn run(command: &mut Command) {
    assert!(
        command.status().expect("Build tool must start").success(),
        "Native HEIC build failed"
    );
}

fn cmake() -> PathBuf {
    println!("cargo:rerun-if-env-changed=CMAKE");
    if let Some(path) = std::env::var_os("CMAKE") {
        return path.into();
    }
    if Command::new("cmake")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return "cmake".into();
    }
    let program_files =
        std::env::var_os("ProgramFiles(x86)").expect("Windows Program Files must exist");
    let output = Command::new(
        Path::new(&program_files).join("Microsoft Visual Studio/Installer/vswhere.exe"),
    )
    .args([
        "-latest",
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.CMake.Project",
        "-property",
        "installationPath",
    ])
    .output()
    .expect("Install CMake or the Visual Studio C++ CMake tools");
    let installation = String::from_utf8(output.stdout).expect("Visual Studio path must be UTF-8");
    let path = Path::new(installation.trim())
        .join("Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe");
    assert!(
        path.is_file(),
        "Install CMake 3.24+ or the Visual Studio C++ CMake tools"
    );
    path
}

fn build_heic() {
    println!("cargo:rerun-if-changed=native");
    let source = std::env::current_dir().unwrap().join("native");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("heic");
    let cmake = cmake();
    run(Command::new(&cmake)
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(&out)
        .args(["-A", "x64"]));
    run(Command::new(cmake).arg("--build").arg(&out).args([
        "--config",
        "Release",
        "--target",
        "lumen_heic",
        "--parallel",
        "4",
    ]));
    println!(
        "cargo:rustc-link-search=native={}",
        out.join("lib").display()
    );
    for library in ["lumen_heic", "heif", "libde265"] {
        println!("cargo:rustc-link-lib=static={library}");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        assert_eq!(
            std::env::var("TARGET").unwrap(),
            "x86_64-pc-windows-msvc",
            "HEIC currently requires Windows x64 MSVC"
        );
        build_heic();
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
