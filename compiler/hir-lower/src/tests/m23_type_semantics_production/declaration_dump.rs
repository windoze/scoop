use scoop_hir as hir;
use scoop_identity::{DeclarationName, SourceDeclarationKey, ValidatedIdentityGraph};

pub(super) fn named(key: &SourceDeclarationKey) -> String {
    let DeclarationName::Named(name) = key.name() else {
        panic!("named source")
    };
    name.as_str().to_owned()
}

pub(super) fn nominal(owner: hir::SourceNominalId, identities: &ValidatedIdentityGraph) -> String {
    match owner {
        hir::SourceNominalId::Concrete(id) => named(
            &identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        ),
        hir::SourceNominalId::GenericTemplate(id) => named(
            &identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        ),
    }
}
