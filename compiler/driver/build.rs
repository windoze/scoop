fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // The Windows 1 MiB default overflows while compiling scoop.core.
    let stack = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        "/STACK:8388608"
    } else {
        "-Wl,--stack,8388608"
    };
    println!("cargo::rustc-link-arg-bin=scoopc={stack}");
}
