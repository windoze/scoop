use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=LLVM_SYS_221_PREFIX");
    println!("cargo:rerun-if-changed=src/debug/records.cpp");
    println!("cargo:rerun-if-changed=src/emission/metadata_object.cpp");
    let config = std::env::var_os("LLVM_SYS_221_PREFIX")
        .map(|prefix| PathBuf::from(prefix).join("bin/llvm-config"))
        .unwrap_or_else(|| PathBuf::from("llvm-config"));
    let version = Command::new(&config)
        .arg("--version")
        .output()
        .expect("LLVM 22.1 llvm-config is required; set LLVM_SYS_221_PREFIX");
    assert!(
        version.status.success() && version.stdout.starts_with(b"22.1."),
        "codegen bindings require LLVM 22.1 headers; set LLVM_SYS_221_PREFIX"
    );
    let output = Command::new(config)
        .arg("--includedir")
        .output()
        .expect("LLVM 22.1 llvm-config is required");
    assert!(output.status.success(), "llvm-config --includedir failed");
    let include = String::from_utf8(output.stdout).expect("LLVM include directory is UTF-8");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .flag("-isystem")
        .flag(include.trim())
        .file("src/debug/records.cpp")
        .file("src/emission/metadata_object.cpp")
        .compile("scoop_llvm_bindings");
}
