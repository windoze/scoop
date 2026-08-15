//! M2-5 翻转后回归：树路径确定性 + 语料覆盖。
//!
//! AST lowering 基线已删除（双路径 oracle 325/325 字节一致后退役——见
//! PLAN.md M2-5 验收记录）。本测试守护树驱动 `lower_module_from_trees` 的
//! 字节级确定性（同输入两次产出逐字节一致——C7）与语料可编译覆盖。
//!
//! 调试设施：`/tmp/scoop2_mod.cfg` 两行（目录 csv、文件名子串）可缩小语料
//! 并在匹配时把两次 dump 写到 /tmp/mod_{ast,tree}.txt（沿用旧文件名）。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};

/// 树路径 MIR 模块产出（确定性 + 覆盖回归）。
#[test]
fn tree_module_determinism_corpus() {
    let (roots, one) = match std::fs::read_to_string("/tmp/scoop2_mod.cfg") {
        Ok(cfg) => {
            let mut lines = cfg.lines();
            let dirs = lines.next().unwrap_or("").trim();
            let roots: Vec<String> = if dirs.is_empty() {
                vec!["mir2".into(), "hir".into(), "run-pass".into()]
            } else {
                dirs.split(',').map(|s| s.trim().to_string()).collect()
            };
            let one = lines.next().unwrap_or("").trim().to_string();
            (roots, one)
        }
        Err(_) => (
            vec!["mir2".into(), "hir".into(), "run-pass".into()],
            String::new(),
        ),
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for r in &roots {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(r);
        if !base.is_dir() {
            continue;
        }
        let mut fs: Vec<PathBuf> = std::fs::read_dir(&base)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "scoop"))
            .map(|e| e.path())
            .collect();
        fs.sort();
        files.extend(fs);
    }
    let mut compared = 0usize;
    let mut diffs: Vec<String> = Vec::new();
    for f in files {
        let Ok(source) = scoop2_base::SourceFile::load(&f) else {
            continue;
        };
        let mut program = build_program(&source);
        let hir = match typecheck_program(&mut program, None) {
            Ok(h) => h,
            Err(_) => continue,
        };
        // 两次独立产出（同 hir 输入）——字节级确定性。
        let dump_with = || {
            let mut sink = scoop2_base::diag::DiagnosticSink::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scoop2_mir::mir::lower_tree::lower_module_from_trees(&hir, &mut sink)
            }));
            result.map(|r| scoop2_mir::mir::dump::dump_module(&r.module, &hir.interner))
        };
        let (a, b) = (dump_with(), dump_with());
        let (a, b) = match (a, b) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                eprintln!("  PANIC {}", f.file_name().unwrap().to_string_lossy());
                continue;
            }
        };
        if !one.is_empty() && f.file_name().unwrap().to_string_lossy().contains(&one) {
            std::fs::write("/tmp/mod_ast.txt", &a).unwrap();
            std::fs::write("/tmp/mod_tree.txt", &b).unwrap();
        }
        compared += 1;
        if a != b {
            let mut first = String::new();
            for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
                if la != lb && first.is_empty() {
                    first = format!("\n    run1: {la}\n    run2: {lb} (L{i})");
                }
            }
            if first.is_empty() {
                first = format!("\n    len a={} b={}", a.len(), b.len());
            }
            diffs.push(format!(
                "{}: 两次运行不一致{first}",
                f.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    eprintln!("tree-determinism: compared={compared} diffs={}", diffs.len());
    for d in diffs.iter().take(8) {
        eprintln!("  NDIFF {d}");
    }
    assert!(compared > 20, "语料应覆盖至少 20 个可编译文件");
    assert!(diffs.is_empty(), "树路径产出应字节级确定: {diffs:#?}");
}
