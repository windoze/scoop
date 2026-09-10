use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{DeclarationAccess, DefinitionOrigin, IntrinsicProviderId, SourceContextId, TypeId};

fn alias() -> TypeAliasDecl {
    TypeAliasDecl {
        name: "Alias".to_string(),
        access: DeclarationAccess::public(),
        target: TypeId::from_raw(0_u32.into()),
        origin: DefinitionOrigin {
            provider: IntrinsicProviderId::from_raw(0),
            file: 0,
            span: Span { start: 0, end: 0 },
            context: SourceContextId::from_raw(0_u32.into()),
        },
    }
}

fn declaration(name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::type_alias(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    )
}

#[test]
fn checked_table_is_total_and_indexed_by_alias_id() {
    let mut aliases = Arena::new();
    let first = aliases.alloc(alias());
    let second = aliases.alloc(alias());
    let identities = HirTypeAliasIdentities::from_declarations(
        &aliases,
        vec![declaration("First"), declaration("Second")],
    )
    .unwrap();

    assert_ne!(identities[first].id(), identities[second].id());
    let scoop_identity::DeclarationName::Named(name) = identities[first].key().name() else {
        panic!("a type alias has a named declaration key")
    };
    assert_eq!(name.as_str(), "First");
}

#[test]
fn checked_table_rejects_missing_identity() {
    let mut aliases = Arena::new();
    aliases.alloc(alias());
    assert!(matches!(
        HirTypeAliasIdentities::from_declarations(&aliases, Vec::new()),
        Err(HirTypeAliasIdentityError::Table(
            HirTypeAliasIdentityTableError::Length {
                expected: 1,
                actual: 0
            }
        ))
    ));
}
