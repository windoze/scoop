use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain,
    GeneratedNominalKey, NormalizedSourcePath, PackagePath, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind,
};

use super::*;

fn source_nominal(name: &str, parameters: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::SourceScoped(
                SourceIdentity::new(
                    ConeIdentity::CORE,
                    NormalizedSourcePath::new("src/types.scoop").unwrap(),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        parameters,
    )
}

#[test]
fn source_and_generated_nominals_keep_distinct_typed_origins() {
    let concrete = HirNominalIdentity::from_source_declaration(source_nominal("Value", 0)).unwrap();
    let generic = HirNominalIdentity::from_source_declaration(source_nominal("Box", 1)).unwrap();
    let generated =
        HirNominalIdentity::from_generated_key(GeneratedNominalKey::ObjectBackingClass {
            object: concrete.concrete_type_id().unwrap(),
        })
        .unwrap();

    assert!(matches!(
        concrete.source(),
        Some(HirSourceNominalIdentity::Concrete(_))
    ));
    assert!(concrete.generic_type_id().is_none());
    assert!(matches!(
        generic.source(),
        Some(HirSourceNominalIdentity::Generic(_))
    ));
    assert!(generic.concrete_type_id().is_none());
    assert!(generated.source().is_none());
    assert!(generated.concrete_type_id().is_some());
}

#[test]
fn aligned_tables_reject_missing_identities_before_indexing() {
    let structs = Arena::new();
    let unexpected =
        HirNominalIdentity::from_source_declaration(source_nominal("Unexpected", 0)).unwrap();

    let error = HirNominalIdentities::checked(
        &structs,
        vec![unexpected],
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(error.table(), HirNominalIdentityTable::Struct);
    assert_eq!(error.expected(), 0);
    assert_eq!(error.actual(), 1);
}

#[test]
fn empty_arenas_do_not_invent_any_source_declaration() {
    let identities = HirNominalIdentities::checked(
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        identities.unit(),
        &CoreBuiltinNominal::Unit.identity_record()
    );
    assert!(
        identities
            .declaration(crate::SourceNominalId::Concrete(
                CoreBuiltinNominal::Any.identity_record().id()
            ))
            .is_none()
    );
}
