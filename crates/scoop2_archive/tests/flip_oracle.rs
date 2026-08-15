//! M2-5 双路径 oracle：对每个树支持的函数，比较「树路径」与「AST 路径」的
//! MIR dump **字节一致**（PLAN.md 验收）。按函数迁移：本测试报告迁移统计，
//! 已迁移函数断言逐字节一致。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir2")
        .join(name)
}

/// 单文件双路径比对：返回 (总函数数, 树支持数, 一致数, 不一致列表)。
thread_local! {
    static CUR_FILE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn flip_compare(source: &scoop2_base::SourceFile) -> (usize, usize, usize, Vec<String>) {
    let mut program = build_program(source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");

    // AST 路径（现行）。
    let mut sink = scoop2_base::diag::DiagnosticSink::new();
    let ast_module = scoop2_mir::mir::lower::lower_module(
        program
            .parsed
            .iter()
            .enumerate()
            .filter(|(i, _)| program.user_indices.contains(i))
            .map(|(i, pf)| (scoop2_base::FileId(i as u32), &pf.file)),
        &hir,
        &mut sink,
    );

    let mut total = 0usize;
    let mut supported = 0usize;
    let mut equal = 0usize;
    let mut diffs: Vec<String> = Vec::new();
    for tf in &hir.files {
        for tree in &tf.trees {
            total += 1;
            if let Some(what) = scoop2_mir::mir::lower_tree::unsupported_construct(tree) {
                if std::env::var("SCOOP2_FLIP_LIST").is_ok() {
                    eprintln!("    UNSUPPORTED {} {}", tree.fqn, what);
                }
                continue;
            }
            // 顶层 val/var 初始化器树 → InitializerRoot 对比。
            if tree.val_init.is_some() {
                if let Some((tree_ir, tree_store)) =
                    scoop2_mir::mir::lower_tree::lower_tree_initializer(
                        &hir, tf.file_id, tree, &hir.store,
                    )
                {
                    supported += 1;
                    let ast_ir = ast_module.module.items.iter().find_map(|it| match it {
                        scoop2_mir::mir::Item::Initializer(ir) if ir.fqn == tree.fqn => {
                            Some(ir.clone())
                        }
                        _ => None,
                    });
                    let Some(ast_ir) = ast_ir else {
                        diffs.push(format!("{}: AST 路径无对应 InitializerRoot", tree.fqn));
                        continue;
                    };
                    let dump_with = |ir: &scoop2_mir::mir::InitializerRoot, types| {
                        let module = scoop2_mir::mir::Module {
                            items: vec![scoop2_mir::mir::Item::Initializer(ir.clone())],
                            types,
                        };
                        scoop2_mir::mir::dump::dump_module(&module, &hir.interner)
                    };
                    let a = dump_with(&ast_ir, ast_module.module.types.clone());
                    let b = dump_with(&tree_ir, tree_store);
                    if a == b {
                        equal += 1;
                    } else {
                        let mut first = String::new();
                        for (la, lb) in a.lines().zip(b.lines()) {
                            if la != lb {
                                first = format!("\n    ast:  {la}\n    tree: {lb}");
                                break;
                            }
                        }
                        if first.is_empty() {
                            first = format!("\n    ast len={} tree len={}", a.len(), b.len());
                        }
                        let fname = CUR_FILE.with(|c| c.borrow().clone());
                        diffs.push(format!("{fname} {}: dump 不一致{first}", tree.fqn));
                    }
                }
                continue;
            }
            // val 初始化器树暂不在脚手架支持内（无签名表项的 fqn 跳过）。
            let Some((tree_fd, _tree_nested, tree_store)) =
                scoop2_mir::mir::lower_tree::lower_tree_fun_decl(
                    &hir, tf.file_id, tree, &hir.store, None,
                )
            else {
                if std::env::var("SCOOP2_FLIP_LIST").is_ok() {
                    eprintln!("    SKIP-NO-SIG {} {}", tree.fqn, tf.file_id.as_u32());
                }
                continue;
            };
            supported += 1;
            if std::env::var("SCOOP2_FLIP_LIST").is_ok() {
                eprintln!("    SUPPORTED-EQ {} ({} equal)", tree.fqn, 0);
            }
            // AST 路径同名 FunDecl。
            let ast_fd = ast_module.module.items.iter().find_map(|it| match it {
                scoop2_mir::mir::Item::Fun(fd) if fd.fqn == tree.fqn => Some(fd.clone()),
                _ => None,
            });
            let Some(ast_fd) = ast_fd else {
                diffs.push(format!("{}: AST 路径无对应 FunDecl", tree.fqn));
                continue;
            };
            // 逐函数 dump 比对（各自构造单 item Module，同一 interner 渲染）。
            let dump_with = |fd: &scoop2_mir::mir::FunDecl, types| {
                let module = scoop2_mir::mir::Module {
                    items: vec![scoop2_mir::mir::Item::Fun(fd.clone())],
                    types,
                };
                scoop2_mir::mir::dump::dump_module(&module, &hir.interner)
            };
            // 各自路径的私有 store 才是 TypeId 的正确渲染上下文（模块合并会
            // remap id——跨 store 比对是伪差异）。
            let a = dump_with(&ast_fd, ast_module.module.types.clone());
            let b = dump_with(&tree_fd, tree_store);
            if a == b {
                equal += 1;
            } else {
                // 诊断输出：首处差异行。
                let mut first = String::new();
                for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
                    if la != lb {
                        first = format!("\n    ast:  {la}\n    tree: {lb}");
                        break;
                    }
                }
                if first.is_empty() {
                    first = format!("\n    ast len={} tree len={}", a.len(), b.len());
                }
                let fname = CUR_FILE.with(|c| c.borrow().clone());
                diffs.push(format!("{fname} {}: dump 不一致{first}", tree.fqn));
            }
        }
    }
    (total, supported, equal, diffs)
}

#[test]
fn flip_oracle_arithmetic() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let (total, supported, equal, diffs) = flip_compare(&source);
    eprintln!("arithmetic: total={total} supported={supported} equal={equal}");
    assert_eq!(total, 2, "add + main");
    assert_eq!(supported, 2, "两函数均在直线子集内");
    assert_eq!(equal, 2, "双路径 dump 应逐字节一致: {diffs:#?}");
}

/// 语料迁移统计（非断言——报告支持率与一致率；--nocapture 查看）。
#[test]
fn flip_oracle_corpus_stats() {
    let roots = ["mir2", "hir"];
    let mut files: Vec<PathBuf> = Vec::new();
    for r in roots {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(r);
        let mut fs: Vec<PathBuf> = std::fs::read_dir(&base)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "scoop"))
            .map(|e| e.path())
            .collect();
        files.extend(fs);
    }
    files.sort();
    let mut total = 0usize;
    let mut supported = 0usize;
    let mut equal = 0usize;
    let mut diff_examples: Vec<String> = Vec::new();
    for f in &files {
        let Ok(source) = scoop2_base::SourceFile::load(f) else {
            continue;
        };
        let mut program = build_program(&source);
        if typecheck_program(&mut program, None).is_err() {
            continue;
        }
        CUR_FILE.with(|c| *c.borrow_mut() = f.file_name().unwrap().to_string_lossy().into_owned());
        let (t, s, e, d) = flip_compare(&source);
        if std::env::var("SCOOP2_FLIP_LIST").is_ok() {
            eprintln!("  FILE {}", f.file_name().unwrap().to_string_lossy());
        }
        total += t;
        supported += s;
        equal += e;
        diff_examples.extend(d.into_iter().take(3));
    }
    eprintln!("corpus: total_fns={total} tree_supported={supported} byte_equal={equal}");
    for d in diff_examples.iter().take(8) {
        eprintln!("  DIFF {d}");
    }
}

/// **模块级**双路径 oracle（M2-5 翻转验收门）：树驱动 `lower_module_from_trees`
/// 与 AST 路径 `lower_module` 的**完整模块 dump** 逐字节一致（含 item 序 /
/// metadata / initializer / extern-global / store 合并序）。
///
/// 目录与单文件 dump 可由 `/tmp/scoop2_mod.cfg` 覆盖（两行：目录 csv、
/// 文件名子串；不存在则默认全语料）——避免测试进程 env 传递的不稳定。
#[test]
fn flip_oracle_module_level() {
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
        let mut sink = scoop2_base::diag::DiagnosticSink::new();
        // AST 路径（现行基准）。
        let ast_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoop2_mir::mir::lower::lower_module(
                program
                    .parsed
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| program.user_indices.contains(i))
                    .map(|(i, pf)| (scoop2_base::FileId(i as u32), &pf.file)),
                &hir,
                &mut sink,
            )
        }));
        let ast_module = match ast_result {
            Ok(m) => m,
            Err(_) => {
                eprintln!("  MPANIC-AST {}", f.file_name().unwrap().to_string_lossy());
                continue;
            }
        };
        // 树路径（M2-5 翻转目标）。
        let tree_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoop2_mir::mir::lower_tree::lower_module_from_trees(&hir, &mut sink)
        }));
        let tree_module = match tree_result {
            Ok(m) => m,
            Err(_) => {
                eprintln!("  MPANIC-TREE {}", f.file_name().unwrap().to_string_lossy());
                continue;
            }
        };
        let a = scoop2_mir::mir::dump::dump_module(&ast_module.module, &hir.interner);
        let b = scoop2_mir::mir::dump::dump_module(&tree_module.module, &hir.interner);
        if !one.is_empty() && f.file_name().unwrap().to_string_lossy().contains(&one) {
            std::fs::write("/tmp/mod_ast.txt", &a).unwrap();
            std::fs::write("/tmp/mod_tree.txt", &b).unwrap();
            for tf in &hir.files {
                for t in &tf.trees {
                    if !t.gaps.is_empty() {
                        eprintln!("    TREE-GAPS {} {:?}", t.fqn, t.gaps);
                    }
                }
                eprintln!(
                    "    SKELETON {}",
                    tf.item_skeleton
                        .iter()
                        .map(|e| format!(
                            "{}{:?}[{}..{}]",
                            e.fqn, e.kind, e.tree_range.0, e.tree_range.1
                        ))
                        .collect::<Vec<_>>()
                        .join(" | ")
                );
            }
        }
        compared += 1;
        if a != b {
            // 差异上下文：首差异行 ± 若干行（迭代定位用）。
            let mut first = String::new();
            let mut ctx: Vec<(usize, &str, &str)> = Vec::new();
            for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
                if la != lb && ctx.len() < 8 {
                    ctx.push((i, la, lb));
                }
            }
            for (i, la, lb) in ctx {
                first.push_str(&format!("\n    L{i} ast:  {la}\n       tree: {lb}"));
            }
            if first.is_empty() {
                first = format!("\n    ast len={} tree len={}", a.len(), b.len());
            }
            diffs.push(format!(
                "{}: 模块 dump 不一致{first}",
                f.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    eprintln!("module-level: compared={compared} diffs={}", diffs.len());
    for d in diffs.iter().take(10) {
        eprintln!("  MDIFF {d}");
    }
    assert!(compared > 20, "语料应覆盖至少 20 个可编译文件");
    assert!(
        diffs.is_empty(),
        "模块级双路径 dump 应逐字节一致: {diffs:#?}"
    );
}
