//! 构建早期 C 运行时（GC/线程/effect TLS 等）。
//!
//! 早期阶段 runtime 用 C 实现以加速落地；后续会逐步迁移到 Scoop 自身。
//! 该 build script 强制使用 clang（见 PLAN.md）。

fn main() {
    let gc_backend = resolve_gc_backend();
    let gc_backend_define = match gc_backend {
        1 => "1", // SCOOP_GC_BACKEND_BASELINE
        2 => "2", // SCOOP_GC_BACKEND_MINIMAL
        3 => "3", // SCOOP_GC_BACKEND_IMMIX
        4 => "4", // SCOOP_GC_BACKEND_HOSTED
        _ => unreachable!("invalid gc backend id: {gc_backend}"),
    };

    // runtime 源码位于 crate 目录之外，需显式声明变更依赖，否则 cargo 无法自动触发重编译。
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_runtime.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_stackmap.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_stackmap.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_array.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_thread.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_once.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_backend_minimal.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_backend_immix.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_backend_hosted.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_common.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_backend.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_root_map_internal.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_stw_internal.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc_immix_internal.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_root_frame.h");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_tls_internal.h");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/platform.h");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/platform_posix.c");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/platform_win32.c");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/unwind.h");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/unwind_posix.c");
    println!("cargo:rerun-if-changed=../../runtime/c/platform/unwind_win32.c");
    println!("cargo:rerun-if-changed=../../sysroot/lib/scoop.runtime.test/native/scoop_test.c");

    // `scoop_once_guard_canonicalize` 在 Linux 需要链接 libdl。
    // macOS 的 dlsym/dlerror 位于 libSystem，无需额外 link-lib。
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=dl");
    }
    // Windows unwind backend（`runtime/c/platform/unwind_win32.c`）依赖 NTDLL 的 Rtl* API。
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=ntdll");
    }

    // cc crate 会把 `compile("name")` 产物作为静态库链接给依赖该 crate 的目标。
    // 注意：driver（编译器）本身不需要链接 runtime；但我们先把 runtime
    // 作为独立构建单元放在这里，后续用于链接用户程序时复用其产物/源码。
    let mut build = cc::Build::new();
    build
        .compiler("clang")
        .define("SCOOP_GC_BACKEND", gc_backend_define)
        .define("SCOOP_RUNTIME_NO_GC_TEST_HELPERS", "1")
        .file("../../runtime/c/scoop_runtime.c")
        .file("../../runtime/c/scoop_stackmap.c")
        .file("../../runtime/c/scoop_array.c")
        .file("../../runtime/c/scoop_thread.c")
        .file("../../runtime/c/scoop_once.c")
        .file("../../runtime/c/scoop_gc_common.c")
        .warnings(true)
        .extra_warnings(true);

    // 为避免产出“空对象文件”触发 ranlib 警告，这里只编译被选中的 backend。
    match gc_backend {
        1 => {
            build.file("../../runtime/c/scoop_gc.c");
        }
        2 => {
            build.file("../../runtime/c/scoop_gc_backend_minimal.c");
        }
        3 => {
            build.file("../../runtime/c/scoop_gc_backend_immix.c");
        }
        4 => {
            build.file("../../runtime/c/scoop_gc_backend_hosted.c");
        }
        _ => unreachable!("invalid gc backend id: {gc_backend}"),
    }

    build.compile("scooprt");

    // A few runtime integration tests exercise GC internals that are intentionally
    // excluded from the core `scooprt` ABI and normal user-program links.
    let mut test_core = cc::Build::new();
    test_core
        .compiler("clang")
        .define("SCOOP_GC_BACKEND", gc_backend_define)
        .include("../../runtime/c")
        .file("../../runtime/c/scoop_runtime.c")
        .file("../../runtime/c/scoop_stackmap.c")
        .file("../../runtime/c/scoop_array.c")
        .file("../../runtime/c/scoop_thread.c")
        .file("../../runtime/c/scoop_once.c")
        .file("../../runtime/c/scoop_gc_common.c")
        .file("../../sysroot/lib/scoop.runtime.test/native/scoop_test.c")
        .warnings(true)
        .extra_warnings(true)
        .cargo_metadata(false);
    match gc_backend {
        1 => {
            test_core.file("../../runtime/c/scoop_gc.c");
        }
        2 => {
            test_core.file("../../runtime/c/scoop_gc_backend_minimal.c");
        }
        3 => {
            test_core.file("../../runtime/c/scoop_gc_backend_immix.c");
        }
        4 => {
            test_core.file("../../runtime/c/scoop_gc_backend_hosted.c");
        }
        _ => unreachable!("invalid gc backend id: {gc_backend}"),
    }
    test_core.compile("scooprt_test_core");
}

fn resolve_gc_backend() -> u8 {
    let baseline = std::env::var("CARGO_FEATURE_GC_BASELINE").is_ok();
    let minimal = std::env::var("CARGO_FEATURE_GC_MINIMAL").is_ok();
    let immix = std::env::var("CARGO_FEATURE_GC_IMMIX").is_ok();
    let hosted = std::env::var("CARGO_FEATURE_GC_HOSTED").is_ok();

    match (baseline, minimal, immix, hosted) {
        (true, false, false, false) => 1,
        (false, true, false, false) => 2,
        (false, false, true, false) => 3,
        (false, false, false, true) => 4,
        (false, false, false, false) => 1, // 未启用特性时回退到 baseline（用于 `--no-default-features`）
        _ => {
            panic!(
                "GC backend features are mutually exclusive; select exactly one of `gc-baseline`, `gc-minimal`, `gc-immix`, or `gc-hosted` (use `--no-default-features` when selecting a non-default backend)"
            );
        }
    }
}
