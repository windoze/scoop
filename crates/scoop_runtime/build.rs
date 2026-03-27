//! 构建早期 C 运行时（GC/线程/effect TLS 等）。
//!
//! 早期阶段 runtime 用 C 实现以加速落地；后续会逐步迁移到 Scoop 自身。
//! 该 build script 强制使用 clang（见 PLAN.md）。

fn main() {
    // runtime 源码位于 crate 目录之外，需显式声明变更依赖，否则 cargo 无法自动触发重编译。
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_runtime.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_task_executor.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc.c");
    println!("cargo:rerun-if-changed=../../runtime/c/scoop_gc.h");

    // cc crate 会把 `compile("name")` 产物作为静态库链接给依赖该 crate 的目标。
    // 注意：driver（编译器）本身不需要链接 runtime；但我们先把 runtime
    // 作为独立构建单元放在这里，后续用于链接用户程序时复用其产物/源码。
    cc::Build::new()
        .compiler("clang")
        .file("../../runtime/c/scoop_runtime.c")
        .file("../../runtime/c/scoop_task_executor.c")
        .file("../../runtime/c/scoop_gc.c")
        .warnings(true)
        .extra_warnings(true)
        .compile("scooprt");
}
