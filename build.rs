fn main() {
    // chat-applefm embeds a Swift bridge whose concurrency runtime is
    // referenced as @rpath/libswift_Concurrency.dylib. Final binaries
    // must carry an rpath to the OS Swift runtime; link-args don't
    // propagate from dependency build scripts, so the binary-producing
    // package emits it. Downstream binary crates using the `applefm`
    // feature need this same line in their own build.rs.
    if std::env::var_os("CARGO_FEATURE_APPLEFM").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
