//! M2 第一刀验收：HIR body 树构造（arithmetic fixture：词汇表封闭、决议内联、
//! gaps 为空——运算符糖展开为 Call）。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};
use scoop2_hir::hir::tree::{FnTree, TreeExprKind};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir2")
        .join(name)
}

#[test]
fn arithmetic_trees_are_gap_free() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");

    let file = &hir.files[0];
    assert_eq!(file.trees.len(), 2, "add + main 两个函数树");
    let add = file
        .trees
        .iter()
        .find(|t| t.fqn.ends_with(".add"))
        .expect("add 树");
    let main = file
        .trees
        .iter()
        .find(|t| t.fqn.ends_with(".main"))
        .expect("main 树");

    for tree in [add, main] {
        assert!(
            tree.gaps.is_empty(),
            "{} 存在构造缺口: {:#?}",
            tree.fqn,
            tree.gaps
        );
    }

    // add：块尾值是 `a + b`——运算符糖必须已展开为 Call（词汇表无 Binary 变体）。
    let add_root = add.body.root.expect("root");
    let tail = add.body.blocks[add_root.0 as usize].tail.expect("add 尾值");
    match &add.body.exprs[tail.0 as usize].kind {
        TreeExprKind::Call { callee, args } => {
            // Method 约定：args 不含接收者（recv 独立）——`a + b` → 1 实参。
            assert_eq!(
                args.len(),
                1,
                "`a + b` 展开为接收者 + 1 实参（args 去接收者）"
            );
            match callee {
                scoop2_hir::hir::tree::TreeCallee::Method { recv, method, .. } => {
                    assert_ne!(*recv, args[0], "接收者独立于实参");
                    let name = hir.interner.resolve(*method);
                    assert!(
                        name.contains("plus") || name.contains("add") || name == "+",
                        "运算符方法名: {name}"
                    );
                }
                other => panic!("`a + b` 应展开为方法调用，实际: {other:?}"),
            }
        }
        other => panic!("add 尾值应为 Call（糖已展开），实际: {other:?}"),
    }

    // main：`val r = add(1, 2)` + `println(r)`。
    let main_root = main.body.root.expect("root");
    let block = &main.body.blocks[main_root.0 as usize];
    assert_eq!(block.stmts.len(), 1, "LocalVal 一条（println 是尾值）");
    match &main.body.stmts[block.stmts[0].0 as usize] {
        scoop2_hir::hir::tree::TreeStmt::LocalVal { local, init } => {
            let name = hir
                .interner
                .resolve(main.body.locals[local.0 as usize].name);
            assert_eq!(name, "r");
            match &main.body.exprs[init.0 as usize].kind {
                TreeExprKind::Call { callee, args } => {
                    assert!(matches!(
                        callee,
                        scoop2_hir::hir::tree::TreeCallee::TopLevel { .. }
                    ));
                    assert_eq!(args.len(), 2);
                    // 实参是两个字面量。
                    for &a in args {
                        assert!(matches!(
                            &main.body.exprs[a.0 as usize].kind,
                            TreeExprKind::Lit(_)
                        ));
                    }
                }
                other => panic!("add(1,2) 应为 TopLevel Call: {other:?}"),
            }
        }
        other => panic!("应为 LocalVal: {other:?}"),
    }
    let tail = block.tail.expect("println 尾值");
    match &main.body.exprs[tail.0 as usize].kind {
        TreeExprKind::Call { args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(matches!(
                &main.body.exprs[args[0].0 as usize].kind,
                TreeExprKind::LocalRef(_)
            ));
        }
        other => panic!("println(r) 应为 Call: {other:?}"),
    }
}

/// 树的 v0 archive 往返：trees 进 collection 且 staged 一致（C8 oracle 的树层）。
#[test]
fn trees_survive_archive_roundtrip() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "scoop2-tree-rt-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");
    scoop2_archive::v0::write_hir_collection(&dir, &program, &hir, &[]).expect("写 archive");
    let loaded = scoop2_archive::v0::load_hir_collection(&dir).expect("装配");

    let direct: Vec<&FnTree> = hir.files[0].trees.iter().collect();
    let roundtrip: Vec<&FnTree> = loaded.hir.files[0].trees.iter().collect();
    assert_eq!(direct.len(), roundtrip.len());
    for (a, b) in direct.iter().zip(roundtrip.iter()) {
        assert_eq!(a.fqn, b.fqn);
        assert_eq!(a.body.exprs.len(), b.body.exprs.len());
        assert_eq!(a.gaps.len(), b.gaps.len());
        assert_eq!(format!("{:?}", a.params), format!("{:?}", b.params));
    }
    std::fs::remove_dir_all(&dir).ok();
}
