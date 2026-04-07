use std::path::PathBuf;

use crate::hir::lower_for_dump;
use crate::session::Session;
use crate::source::SourceFile;
use crate::typecheck::TypeEnv;

use super::export_public_api_for_source;

#[test]
fn scoopir_fixture_public_api_filter_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/scoopir/public_api_filter.scoop");
    let source = SourceFile::load(&fixture_path).unwrap();

    let ast = sess.parse(&source).unwrap();
    let index = sess.build_top_level_index(&[source.clone()]).unwrap();
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
    let index = sess.build_top_level_index(&[source.clone()]).unwrap();
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
