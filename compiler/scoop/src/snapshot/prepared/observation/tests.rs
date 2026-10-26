use super::*;
use scoop_protocol::{HostPathCarrier, StageDumpKindV1};

#[test]
fn stores_requested_dumps_and_finds_only_missing_stages() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("cache");
    let source = root.path().join("source.txt");
    std::fs::write(&source, b"saved AST").unwrap();
    let ast = StageDumpSet::one(StageDumpKindV1::Ast);
    let descriptors = [EmittedDumpDescriptorV1::new(
        StageDumpKindV1::Ast,
        EmittedDumpDestinationV1::File(HostPathCarrier::from_path(&source).unwrap()),
    )];
    assert_eq!(missing_stages(ast, &cache).unwrap(), Some(ast));
    store_dumps(Some(ast), &descriptors, &cache).unwrap();
    assert_eq!(missing_stages(ast, &cache).unwrap(), None);
    let missing = missing_stages(StageDumpSet::all(), &cache)
        .unwrap()
        .unwrap();
    assert_eq!(
        missing.iter().collect::<Vec<_>>(),
        vec![
            StageDumpKindV1::Hir,
            StageDumpKindV1::Mir,
            StageDumpKindV1::Optimizations,
            StageDumpKindV1::Lir,
            StageDumpKindV1::Symbols
        ]
    );
    assert_eq!(std::fs::read(cache.join("ast.txt")).unwrap(), b"saved AST");
    assert!(store_dumps(Some(StageDumpSet::all()), &descriptors, &cache).is_err());
}
