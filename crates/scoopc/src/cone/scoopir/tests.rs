use std::path::PathBuf;

use crate::cone::{ConeManifest, ConeNativeBuildConfig, ConeSection};
use crate::hir::lower_for_dump;
use crate::session::Session;
use crate::source::SourceFile;
use crate::typecheck::TypeEnv;

use super::{export_public_api_for_cone_sources, export_public_api_for_source};

fn test_manifest(name: &str) -> ConeManifest {
    ConeManifest {
        cone: ConeSection {
            name: name.to_string(),
            version: "0.1.0".to_string(),
        },
        dependencies: Default::default(),
        pre_specialize_functions: Vec::new(),
        pre_specialize_types: Vec::new(),
        export_entry_points: Vec::new(),
        selectors: Vec::new(),
        native_build: ConeNativeBuildConfig::default(),
    }
}

#[test]
fn scoopir_fixture_public_api_filter_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/scoopir/public_api_filter.scoop");
    let source = SourceFile::load(&fixture_path).unwrap();

    let ast = sess.parse(&source).unwrap();
    let index = sess
        .build_top_level_index(std::slice::from_ref(&source))
        .unwrap();
    let mut env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();

    let hir = lower_for_dump(&sess, &source).unwrap();

    let ir = export_public_api_for_source(&source, &index, &env, &hir).unwrap();
    let actual = format!("{}\n", serde_json::to_string_pretty(&ir).unwrap());

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/scoopir/public_api_filter.scoopir.json");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn scoopir_fixture_package_level_comptime_if_public_api_trimmed_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/scoopir/package_level_comptime_if_public_api_trimmed.scoop");
    let source = SourceFile::load(&fixture_path).unwrap();

    let ast = sess.parse(&source).unwrap();
    let index = sess
        .build_top_level_index(std::slice::from_ref(&source))
        .unwrap();
    let mut env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();

    let hir = lower_for_dump(&sess, &source).unwrap();

    let ir = export_public_api_for_source(&source, &index, &env, &hir).unwrap();
    let actual = format!("{}\n", serde_json::to_string_pretty(&ir).unwrap());

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/fixtures/scoopir/package_level_comptime_if_public_api_trimmed.scoopir.json",
    );
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn export_public_api_for_cone_sources_trims_package_level_comptime_if_across_files() {
    let sess = Session::new().unwrap();
    let defs = SourceFile::new_virtual(
        "<defs>",
        "package fixtures.scoopir.multi\nimport scoop.core.*\nconst fun truthy<T>(value: T): Bool { return true }\n",
    );
    let main = SourceFile::new_virtual(
        "<main>",
        "package fixtures.scoopir.multi\nimport scoop.core.*\ncomptime if (truthy<Int>(1)) {\n    public fun selected(): Int { return 7 }\n}\n",
    );

    let manifest = test_manifest("fixtures-scoopir-multi");
    let ir = export_public_api_for_cone_sources(&sess, &[defs, main], &manifest).unwrap();
    assert!(
        ir.funs
            .iter()
            .any(|fun| fun.fqn == "fixtures.scoopir.multi.selected"),
        "被选中的 package-level comptime if 分支应出现在多源 cone 导出的 public API 中"
    );
}
