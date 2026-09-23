use scoop_hir as hir;
use scoop_identity::{
    DeclarationName, SignatureTypeKey, SourceDeclarationKey, ValidatedIdentityGraph,
};

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

pub(super) fn ty(value: &SignatureTypeKey, identities: &ValidatedIdentityGraph) -> String {
    match value {
        SignatureTypeKey::Nominal(id) => nominal(hir::SourceNominalId::Concrete(*id), identities),
        SignatureTypeKey::NominalApplication { origin, arguments } => format!(
            "{}<{}>",
            nominal(hir::SourceNominalId::GenericTemplate(*origin), identities),
            arguments
                .as_slice()
                .iter()
                .map(|v| ty(v, identities))
                .collect::<Vec<_>>()
                .join(",")
        ),
        SignatureTypeKey::Binder { depth, index } => format!("binder({depth},{index})"),
        other => panic!("unexpected fixture signature {other:?}"),
    }
}
