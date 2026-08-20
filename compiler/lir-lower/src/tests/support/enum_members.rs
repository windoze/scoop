use super::super::*;

pub(in crate::tests) fn test_variant(
    name: String,
    gc_free: bool,
    fields: Vec<mir::Field>,
) -> mir::VariantDef {
    let owner = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("TestEnum").unwrap(),
        scoop_identity::SourceNominalKind::Enum,
        0,
    );
    let key = scoop_identity::EnumVariantIdentityKey::source(
        &owner,
        CanonicalIdentifier::new(&name).unwrap(),
    )
    .unwrap();
    let identity = scoop_identity::PersistentEnumVariantId::from_key(&key).unwrap();
    mir::VariantDef {
        identity,
        name,
        gc_free,
        fields: fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                let key = scoop_identity::EnumVariantFieldKey::new(
                    identity,
                    scoop_identity::EnumVariantFieldSelector::Positional {
                        declaration_index: u32::try_from(index).unwrap(),
                    },
                );
                mir::VariantField {
                    identity: scoop_identity::PersistentEnumVariantFieldId::from_key(&key).unwrap(),
                    name: field.name,
                    ty: field.ty,
                }
            })
            .collect(),
    }
}
