fn main() {
    println!("cargo:rerun-if-changed=../../src/permissions/macos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64",
            Ok("x86_64") => "x86_64",
            _ => panic!("unsupported macOS architecture"),
        };
        let object = out.join("permissions.o");
        let compiled = std::process::Command::new("xcrun")
            .env("CLANG_MODULE_CACHE_PATH", out.join("clang-modules"))
            .args([
                "clang",
                "-fobjc-arc",
                "-fmodules",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-mmacosx-version-min=14.0",
                "-arch",
                arch,
                "-c",
                "../../src/permissions/macos.m",
                "-o",
            ])
            .arg(&object)
            .status()
            .expect("compile permission companion with Xcode command line tools");
        assert!(compiled.success(), "permission companion failed to compile");
        let archived = std::process::Command::new("xcrun")
            .args(["ar", "crs"])
            .arg(out.join("libkudzu_permissions.a"))
            .arg(object)
            .status()
            .expect("archive permission companion");
        assert!(archived.success(), "permission companion failed to archive");
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=kudzu_permissions");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }
}
