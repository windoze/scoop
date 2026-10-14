use super::*;
use scoop_identity::SourceRole;

#[test]
fn test_source_parsing_preserves_roles_and_reports_the_original_source() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::create_dir(root.join("cases")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        "schema = 1\n\
        [cone]\ngroup = 'test'\nname = 'source-roles'\nversion = '0.0.0'\nkind = 'library'\n\
        [[test.sources]]\npath = 'cases'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/api.scoop"), "fun api() {}\n").unwrap();
    std::fs::write(root.join("cases/test.scoop"), "fun check() { api() }\n").unwrap();
    let manifest = load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap();
    let target = scoop_target_data::DARWIN_AARCH64_ID;
    let production =
        parse_manifest_current(&manifest, target, CompilationPurpose::Production).unwrap();
    let tests = parse_manifest_current(&manifest, target, CompilationPurpose::Test).unwrap();
    let sources = tests.iter().collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].source().role(), SourceRole::Test);
    assert_eq!(
        sources[0].source().identity().logical_path().as_str(),
        "cases/test.scoop"
    );
    assert_eq!(sources[1].source().role(), SourceRole::Production);
    assert_eq!(sources[1].source(), production.sources().sources().first());
    assert_eq!(tests.cone(), production.cone());

    std::fs::write(root.join("cases/test.scoop"), "fun broken(\n").unwrap();
    parse_manifest_current(&manifest, target, CompilationPurpose::Production).unwrap();
    let CurrentConeSourceStageError::Parser(error) =
        parse_manifest_current(&manifest, target, CompilationPurpose::Test).unwrap_err()
    else {
        panic!("the test source must fail during parsing");
    };
    let ParseCurrentConeError::Diagnostics(diagnostics) = *error else {
        panic!("expected syntax diagnostics");
    };
    assert!(!diagnostics.is_empty());
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.identity() == sources[0].source().identity())
    );
}
