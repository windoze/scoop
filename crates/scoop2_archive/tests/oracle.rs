//! M1 oracle（PLAN.md C8）：one-shot 与 staged 管线的 MIR dump **字节一致**；
//! staged 只读 archive 目录（源文件可删）；archive 字节级确定（同输入两次落地
//! hash 一致）；版本头不匹配即拒（C7）。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};
use scoop2_archive::v0;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir2")
        .join(name)
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "scoop2-oracle-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    d
}

/// one-shot：parse → typecheck → MIR（不经过任何序列化）。
fn one_shot(source: &scoop2_base::SourceFile) -> String {
    let mut program = build_program(source);
    let hir = typecheck_program(&mut program, None).expect("typecheck 应成功");
    let mir_files: Vec<(scoop2_base::FileId, &scoop2_syntax::ast::File)> = program
        .parsed
        .iter()
        .enumerate()
        .filter(|(i, _)| program.user_indices.contains(i))
        .map(|(i, pf)| (scoop2_base::FileId(i as u32), &pf.file))
        .collect();
    v0::run_mir_and_dump(&hir, &mir_files).expect("one-shot MIR 应成功")
}

/// staged：parse → typecheck → 写 archive → 【只读目录】装配 → MIR。
fn staged(archive_dir: &std::path::Path, source: &scoop2_base::SourceFile) -> String {
    let mut program = build_program(source);
    let hir = typecheck_program(&mut program, None).expect("typecheck 应成功");
    v0::write_hir_collection(archive_dir, &program, &hir, &[]).expect("写 archive");
    let loaded = v0::load_hir_collection(archive_dir).expect("装配 archive");
    v0::mir_dump_from_collection(&loaded).expect("staged MIR 应成功")
}

#[test]
fn oracle_one_shot_matches_staged() {
    for name in ["arithmetic.scoop"] {
        let source = scoop2_base::SourceFile::load(&fixture(name)).expect("读 fixture");
        let dir = tmp_dir("eq");
        let direct = one_shot(&source);
        let roundtrip = staged(&dir, &source);
        assert!(!direct.is_empty(), "{name}: dump 不应为空");
        assert_eq!(
            direct, roundtrip,
            "{name}: one-shot 与 staged（序列化往返）输出分叉"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn staged_runs_without_sources() {
    // 把 fixture 复制到临时文件 → 落地 archive → 删除源文件 → staged MIR 仍可跑。
    let dir = tmp_dir("nosrc");
    std::fs::create_dir_all(&dir).unwrap();
    let src_copy = dir.join("main.scoop");
    std::fs::copy(fixture("arithmetic.scoop"), &src_copy).unwrap();
    let source = scoop2_base::SourceFile::load(&src_copy).unwrap();
    let archive_dir = dir.join("archives");
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");
    v0::write_hir_collection(&archive_dir, &program, &hir, &[]).expect("写 archive");

    std::fs::remove_file(&src_copy).unwrap(); // 删除源文件
    let loaded = v0::load_hir_collection(&archive_dir).expect("装配（不读源）");
    let dump = v0::mir_dump_from_collection(&loaded).expect("staged MIR（无源文件）");
    assert!(!dump.is_empty());

    // 与 one-shot 输出仍一致。
    let source2 = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    assert_eq!(dump, one_shot(&source2));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn archive_bytes_are_deterministic() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let dir1 = tmp_dir("det1");
    let dir2 = tmp_dir("det2");
    for dir in [&dir1, &dir2] {
        let mut program = build_program(&source);
        let hir = typecheck_program(&mut program, None).expect("typecheck");
        v0::write_hir_collection(dir, &program, &hir, &[]).expect("写 archive");
    }
    let files1 = sorted_file_names(&dir1);
    let files2 = sorted_file_names(&dir2);
    assert_eq!(files1, files2, "两次落地文件集合应一致");
    for name in files1 {
        let a = std::fs::read(dir1.join(&name)).unwrap();
        let b = std::fs::read(dir2.join(&name)).unwrap();
        assert_eq!(a, b, "文件 {name} 字节应一致（C7：无 HashMap 序泄漏）");
    }
    std::fs::remove_dir_all(&dir1).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn load_rejects_version_mismatch() {
    let source = scoop2_base::SourceFile::load(&fixture("arithmetic.scoop")).unwrap();
    let dir = tmp_dir("ver");
    let mut program = build_program(&source);
    let hir = typecheck_program(&mut program, None).expect("typecheck");
    v0::write_hir_collection(&dir, &program, &hir, &[]).expect("写 archive");

    // 篡改首个 cone archive 的 schema_version（重新编码）。
    let cone_file = sorted_file_names(&dir)
        .into_iter()
        .find(|n| n.ends_with(v0::CONE_EXT))
        .expect("应有 cone archive");
    let path = dir.join(&cone_file);
    let bytes = std::fs::read(&path).unwrap();
    let (mut archive, _): (v0::HirConeArchive, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    archive.header.schema_version = u32::MAX;
    let tampered = bincode::serde::encode_to_vec(&archive, bincode::config::standard()).unwrap();
    std::fs::write(&path, tampered).unwrap();

    let err = v0::load_hir_collection(&dir).expect_err("版本不匹配应被拒绝");
    assert!(
        matches!(err, v0::ArchiveError::VersionMismatch { .. }),
        "应为 VersionMismatch，实际: {err}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn sorted_file_names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
        .collect();
    names.sort_unstable();
    names
}
