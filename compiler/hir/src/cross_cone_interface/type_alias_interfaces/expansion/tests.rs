use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin, DefinitionOwnerChain,
    NormalizedSourcePath, PackagePath, PersistentTypeAliasId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::WirePath;

use super::*;
use crate::{ExportDefinitionSourceV1, PublicLookupAccessV1};

#[test]
fn expands_local_and_external_chains_with_shared_memoized_targets() {
    let first = alias("First");
    let second = alias("Second");
    let bridge = alias("Bridge");
    let third = alias("Third");
    let target = nominal("Target");
    let path = WirePath::root();
    let provider = CanonicalTypeAliasInterfacesV1::try_new(vec![alias_record(
        third,
        TypeAliasTargetV1::Signature(SignatureTypeKey::Nominal(target)),
    )])
    .unwrap()
    .expand_alias_closure(&[], &path)
    .unwrap();
    let intermediate = CanonicalTypeAliasInterfacesV1::try_new(vec![alias_record(
        bridge,
        TypeAliasTargetV1::Alias(third),
    )])
    .unwrap()
    .expand_alias_closure(&[&provider], &path)
    .unwrap();
    let consumer = CanonicalTypeAliasInterfacesV1::try_new(vec![
        alias_record(first, TypeAliasTargetV1::Alias(bridge)),
        alias_record(second, TypeAliasTargetV1::Alias(first)),
    ])
    .unwrap()
    .expand_alias_closure(&[&intermediate], &path)
    .unwrap();
    assert_eq!(consumer.entries().len(), 2);
    assert!(!consumer.is_empty());
    let first = consumer.get(first).unwrap();
    let second = consumer.get(second).unwrap();
    assert_eq!(first.target(), &SignatureTypeKey::Nominal(target));
    assert!(std::ptr::eq(first.target(), second.target()));
    assert!(std::ptr::eq(
        first.target(),
        provider.get(third).unwrap().target()
    ));
}

#[test]
fn rejects_a_missing_alias_target() {
    let missing = alias("Missing");
    let table = CanonicalTypeAliasInterfacesV1::try_new(vec![alias_record(
        alias("Root"),
        TypeAliasTargetV1::Alias(missing),
    )])
    .unwrap();
    assert_eq!(
        table.expand_alias_closure(&[], &WirePath::root()),
        Err(TypeAliasExpansionError::MissingInterface { alias: missing })
    );
}

#[test]
fn reports_the_complete_cycle_with_repeated_terminal_node() {
    let a = alias("CycleA");
    let b = alias("CycleB");
    let c = alias("CycleC");
    let table = CanonicalTypeAliasInterfacesV1::try_new(vec![
        alias_record(a, TypeAliasTargetV1::Alias(b)),
        alias_record(b, TypeAliasTargetV1::Alias(c)),
        alias_record(c, TypeAliasTargetV1::Alias(a)),
    ])
    .unwrap();
    let Err(TypeAliasExpansionError::Cycle { chain }) =
        table.expand_alias_closure(&[], &WirePath::root())
    else {
        panic!("cyclic aliases must be rejected");
    };
    assert_eq!(chain.len(), 4);
    assert_eq!(chain.first(), chain.last());
    assert_eq!(chain[0], table.records()[0].alias());
    for (current, next) in chain.iter().zip(&chain[1..]) {
        assert_eq!(
            table.get(*current).unwrap().target(),
            &TypeAliasTargetV1::Alias(*next)
        );
    }
}

fn alias_record(
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
) -> TypeAliasInterfaceRecordV1 {
    TypeAliasInterfaceRecordV1::try_new(
        alias,
        target,
        PublicLookupAccessV1::DirectOnly,
        definition_origin(),
    )
    .unwrap()
}

fn definition_origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Aliases.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn alias(name: &str) -> PersistentTypeAliasId {
    PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        site(),
        identifier(name),
    ))
    .unwrap()
}

fn nominal(name: &str) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site(),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
