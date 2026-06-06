//! Compiles the Swift bridge (`bridge/`) and links it into the crate.
//!
//! The bridge is the only piece of this provider that talks to Apple's
//! FoundationModels framework — see `bridge/Sources/AppleFMBridge`. It is
//! built with SwiftPM into a static library and linked here, so users only
//! ever run `cargo build`.
//!
//! On non-macOS targets (or when `APPLEFM_SKIP_BRIDGE` is set, or on
//! docs.rs) the bridge is not built and the crate falls back to a stub
//! that reports the model as unavailable. This keeps the workspace
//! compiling everywhere without Xcode.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(applefm_bridge)");
    println!("cargo:rerun-if-changed=bridge/Sources");
    println!("cargo:rerun-if-changed=bridge/Package.swift");
    println!("cargo:rerun-if-env-changed=APPLEFM_SKIP_BRIDGE");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let docsrs = env::var("DOCS_RS").is_ok();
    let skip = env::var("APPLEFM_SKIP_BRIDGE").is_ok();
    if target_os != "macos" || docsrs || skip {
        // Stub mode: `cfg(applefm_bridge)` is not emitted.
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let scratch = out_dir.join("bridge-build");

    let status = Command::new("swift")
        .arg("build")
        .args(["-c", "release"])
        .arg("--package-path")
        .arg(manifest_dir.join("bridge"))
        .arg("--scratch-path")
        .arg(&scratch)
        .status()
        .expect(
            "failed to invoke `swift build` — building chat-applefm on macOS \
             requires the Xcode command line tools (xcode-select --install)",
        );
    assert!(
        status.success(),
        "swift build of the AppleFMBridge package failed"
    );

    println!(
        "cargo:rustc-link-search=native={}",
        scratch.join("release").display()
    );
    println!("cargo:rustc-link-lib=static=AppleFMBridge");

    // Swift runtime stubs from the SDK so the autolinked -lswiftCore etc.
    // resolve at link time. At run time the OS runtime in /usr/lib/swift
    // is used.
    let sdk_path = Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .expect("failed to invoke `xcrun --show-sdk-path`");
    let sdk_path = String::from_utf8_lossy(&sdk_path.stdout).trim().to_string();
    println!("cargo:rustc-link-search=native={sdk_path}/usr/lib/swift");
    println!("cargo:rustc-link-search=native=/usr/lib/swift");

    // Weak-link so binaries still load on macOS < 26; the bridge guards
    // every use behind `#available` and reports unavailability instead.
    println!("cargo:rustc-link-arg=-Wl,-weak_framework,FoundationModels");

    // The Swift concurrency runtime is referenced as
    // @rpath/libswift_Concurrency.dylib; point the rpath at the OS
    // runtime so binaries resolve it from the dyld shared cache.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    println!("cargo:rustc-cfg=applefm_bridge");
}
