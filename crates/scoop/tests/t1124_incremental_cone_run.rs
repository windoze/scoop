#![cfg(all(feature = "llvm", not(windows)))]
//! T1124：Cone 粗粒度增量构建（`scoop run` cache hit）。
//!
//! 说明：这里用“真实 scoop 二进制（`CARGO_BIN_EXE_scoop`）”做黑盒回归：
//! - 第一次 `scoop run`：应正常构建并运行；
//! - 第二次 `scoop run`：应命中 build.json 的 fingerprint，打印 cache hit 并跳过构建。

use std::process::Command;

use tempfile::tempdir;

#[test]
fn scoop_run_cache_hit_skips_build_and_runs_program() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("app");
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("Cone.toml"),
        r#"
[cone]
name = "fixture-incremental"
version = "0.0.0"
"#,
    )
    .unwrap();

    std::fs::write(
        root.join("src/main.scoop"),
        r#"
package fixtures.incremental

import scoop.core.*

public fun main() / Pure! {
    println("ok")
}
"#,
    )
    .unwrap();

    let scoop = env!("CARGO_BIN_EXE_scoop");

    let out1 = Command::new(scoop).arg("run").arg(&root).output().unwrap();
    assert!(out1.status.success(), "第一次 run 应成功：{out1:?}");
    assert_eq!(String::from_utf8_lossy(&out1.stdout), "ok\n");
    let err1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        !err1.contains("skipping build (cache hit)"),
        "第一次 run 不应命中缓存，stderr={err1}"
    );

    assert!(
        root.join("build/debug/build.json").is_file(),
        "第一次 run 后应写出 build.json"
    );

    let out2 = Command::new(scoop).arg("run").arg(&root).output().unwrap();
    assert!(out2.status.success(), "第二次 run 应成功：{out2:?}");
    assert_eq!(String::from_utf8_lossy(&out2.stdout), "ok\n");
    let err2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        err2.contains("skipping build (cache hit)"),
        "第二次 run 应命中缓存并提示跳过构建，stderr={err2}"
    );

    // 禁用开关：不应命中 cache hit（即使 build.json + exe 已存在）。
    let out3 = Command::new(scoop)
        .arg("run")
        .arg("--no-incremental")
        .arg(&root)
        .output()
        .unwrap();
    assert!(out3.status.success(), "禁用增量后 run 仍应成功：{out3:?}");
    assert_eq!(String::from_utf8_lossy(&out3.stdout), "ok\n");
    let err3 = String::from_utf8_lossy(&out3.stderr);
    assert!(
        !err3.contains("skipping build (cache hit)"),
        "禁用增量时不应命中缓存，stderr={err3}"
    );
}
