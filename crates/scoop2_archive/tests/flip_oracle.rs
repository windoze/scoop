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
            if scoop2_mir::mir::lower_tree::unsupported_construct(tree).is_some() {
                continue;
            }
            // val 初始化器树暂不在脚手架支持内（无签名表项的 fqn 跳过）。
            let Some((tree_fd, tree_store)) = scoop2_mir::mir::lower_tree::lower_tree_fun_decl(
                &hir, tf.file_id, tree, &hir.store,
            ) else {
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
