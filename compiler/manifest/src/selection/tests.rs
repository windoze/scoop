use super::*;
use crate::test_catalog::parse_cone_manifest;

const CONE: &str =
    "[cone]\ngroup = 'dev.selection'\nname = 'test'\nversion = '1.0.0'\nkind = 'library'\n";

fn selection(source: &str) -> SourceSelection {
    parse_cone_manifest(&format!("schema = 1\n{source}\n{CONE}"))
        .unwrap()
        .semantic()
        .sources()
        .clone()
}

#[test]
fn missing_and_explicit_empty_sources_are_different() {
    assert_eq!(selection(""), SourceSelection::Default);
    assert_eq!(
        selection("sources = []"),
        SourceSelection::Explicit(Vec::new())
    );
}

#[test]
fn predicates_use_target_os_arch_and_libc_with_and_or_semantics() {
    let SourceSelection::Explicit(paths) = selection(
        "[[sources]]\npath = 'common'\nwhen = {os = ['darwin', 'linux'], arch = 'x86_64', env = ['gnu', 'musl']}\n\
         [[sources]]\npath = 'gnu'\nwhen = {os = 'linux', env = 'gnu'}\n\
         [[sources]]\npath = 'nothing'\nwhen = {arch = []}",
    ) else {
        panic!("explicit selection expected")
    };
    assert!(
        !paths[0]
            .predicate
            .matches(scoop_target_data::DARWIN_AARCH64_ID)
    );
    assert!(
        paths[0]
            .predicate
            .matches(scoop_target_data::LINUX_X86_64_GNU_ID)
    );
    assert!(
        paths[0]
            .predicate
            .matches(scoop_target_data::LINUX_X86_64_MUSL_ID)
    );
    assert!(
        paths[1]
            .predicate
            .matches(scoop_target_data::LINUX_X86_64_GNU_ID)
    );
    assert!(
        !paths[1]
            .predicate
            .matches(scoop_target_data::LINUX_X86_64_MUSL_ID)
    );
    assert!(
        crate::test_catalog::SOURCE_TARGETS
            .iter()
            .all(|target| !paths[2].predicate.matches(target.id))
    );
}

#[test]
fn malformed_entries_are_rejected_even_with_a_false_predicate() {
    for entry in [
        "path = '../outside'\nwhen = {os = []}",
        "path = '/outside'\nwhen = {os = []}",
        "path = ''\nwhen = {os = []}",
        "path = 'src/**/*.scoop'\nwhen = {os = []}",
        "path = 'src'\nwhen = {os = [], env = 'glibc'}",
        "path = 'src'\nwhen = {cpu = 'x86_64'}",
        "path = 'src'\nwhen = {os = 1}",
        "path = 'src'\nwhen = {os = ['linux', 1]}",
        "path = 'src'\nwhen = false",
        "path = 'src'\nunknown = true",
        "path = 42",
        "when = {os = 'linux'}",
    ] {
        let source = format!("schema = 1\n[[sources]]\n{entry}\n{CONE}");
        let error = parse_cone_manifest(&source).unwrap_err();
        assert!(error.span().is_some(), "{entry}: {error}");
    }
}

#[test]
fn normalizes_relative_paths_without_filesystem_access() {
    for (input, expected) in [("./src//a/../common/", "src/common"), ("a/..", ".")] {
        assert_eq!(ConeRelativePath::new(input).unwrap().as_str(), expected);
    }
    for input in [
        "../outside",
        "a/../../b",
        "/a",
        "C:/a",
        "./C:/a",
        "a/../C:/b",
        "C:/../b",
        "a\\b",
        "a\0b",
    ] {
        assert!(ConeRelativePath::new(input).is_err(), "{input:?}");
    }
    assert_eq!(
        ConeRelativePath::new("data:files/api.scoop")
            .unwrap()
            .as_str(),
        "data:files/api.scoop"
    );
}

#[test]
fn cache_projection_ignores_entry_order_and_equivalent_predicate_spelling() {
    let first = selection(
        "[[sources]]\npath = './src/common'\n[[sources]]\npath = 'linux'\nwhen = {os = 'linux'}",
    );
    let reordered = selection(
        "[[sources]]\npath = 'linux'\nwhen = {env = ['musl', 'gnu', 'gnu']}\n[[sources]]\npath = 'src/common'",
    );
    assert_eq!(
        scoop_wire::encode(&first).unwrap(),
        scoop_wire::encode(&reordered).unwrap()
    );
    assert_ne!(
        scoop_wire::encode(&first).unwrap(),
        scoop_wire::encode(&selection("")).unwrap()
    );
}

#[test]
fn predicate_validation_uses_the_supplied_catalog_and_canonicalizes_its_order() {
    let source = format!(
        "schema = 1\nsources = [{{ path = \"linux.scoop\", when = {{ os = \"linux\" }} }}]\n{CONE}"
    );
    let catalog = crate::test_catalog::SOURCE_TARGETS;
    let error = crate::parse_cone_manifest(&source, &catalog[..1]).unwrap_err();
    assert!(error.to_string().contains("unknown target predicate"));
    let normal = crate::parse_cone_manifest(&source, catalog).unwrap();
    let mut reversed = catalog.to_vec();
    reversed.reverse();
    let reordered = crate::parse_cone_manifest(&source, &reversed).unwrap();
    assert_eq!(
        scoop_wire::encode(normal.semantic().sources()).unwrap(),
        scoop_wire::encode(reordered.semantic().sources()).unwrap(),
    );
}
