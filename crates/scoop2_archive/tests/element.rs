//! M2-1 验收：声明层 element 表（span 非零、索引、确定性、v0 archive 往返）。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};
use scoop2_hir::hir::element::ElementKind;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir2")
        .join(name)
}

#[test]
fn elements_for_arithmetic() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");

    // 顶层函数 element：decl_span 非零（此前恒 0..0 的缺口）。
    let fqn = hir.interner.get("fixtures.mir2.add").unwrap();
    let ids = hir.elements.overloads(fqn);
    assert_eq!(ids.len(), 1, "add 单实现");
    let e = &hir.elements.elements[ids[0].0 as usize];
    assert!(matches!(e.kind, ElementKind::Fun { .. }));
    assert!(e.decl_span.end > e.decl_span.start, "decl_span 应非零");

    // println（sysroot）也应有 element + 非零 span。
    let println_fqn = hir
        .interner
        .get("scoop.core.__scoop_print")
        .or(hir.interner.get("println"))
        .unwrap();
    let ids = hir.elements.overloads(println_fqn);
    assert!(!ids.is_empty(), "sysroot 函数也入表");
    for &id in ids {
        let e = &hir.elements.elements[id.0 as usize];
        assert!(matches!(e.kind, ElementKind::Fun { .. }));
    }

    // 确定性：同输入两次装配，序列化字节一致。
    let mut program2 = build_program(&source);
    let hir2 = typecheck_program(&mut program2, None).unwrap();
    let b1 = bincode::serde::encode_to_vec(&hir.elements, bincode::config::standard()).unwrap();
    let b2 = bincode::serde::encode_to_vec(&hir2.elements, bincode::config::standard()).unwrap();
    assert_eq!(b1, b2, "element 表字节确定");
}

#[test]
fn elements_for_enum_fixture() {
    let source = scoop2_base::SourceFile::load(&fixture("enum_when.scoop")).unwrap();
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");

    // enum 类型 element + variant element。
    let color = hir.interner.get("fixtures.mir2.Color").unwrap();
    let has_type = hir.elements.overloads(color).iter().any(|&id| {
        matches!(
            hir.elements.elements[id.0 as usize].kind,
            ElementKind::Type { .. }
        )
    });
    assert!(has_type, "Color Type element 存在");
    let variants = hir
        .elements
        .overloads(color)
        .iter()
        .filter(|&&id| {
            matches!(
                hir.elements.elements[id.0 as usize].kind,
                ElementKind::EnumVariant
            )
        })
        .count();
    assert_eq!(variants, 3, "Red/Green/Blue 三个 variant element");
}

#[test]
fn elements_survive_archive_roundtrip() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "scoop2-elem-rt-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).unwrap();
    scoop2_archive::v0::write_hir_collection(&dir, &program, &hir, &[]).unwrap();
    let loaded = scoop2_archive::v0::load_hir_collection(&dir).unwrap();
    let direct = bincode::serde::encode_to_vec(&hir.elements, bincode::config::standard()).unwrap();
    let roundtrip =
        bincode::serde::encode_to_vec(&loaded.hir.elements, bincode::config::standard()).unwrap();
    assert_eq!(direct, roundtrip, "element 表 archive 往返一致");
    // 反序列化重建的索引可用。
    let fqn = loaded.hir.interner.get("fixtures.mir2.main").unwrap();
    assert_eq!(loaded.hir.elements.overloads(fqn).len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}
