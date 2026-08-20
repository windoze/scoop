use super::*;
use scoop_identity::{PersistentGenericTypeId, SourceDeclarationKey, SourceNominalKind};

pub(super) fn write_source(root: &Path) {
    for (name, source) in [
        (
            "standalone",
            include_str!(
                "../../../../../../../tests/fixtures/m23-reference-source-fields/standalone.scoop"
            ),
        ),
        (
            "combined",
            include_str!(
                "../../../../../../../tests/fixtures/m23-reference-source-fields/combined.scoop"
            ),
        ),
    ] {
        std::fs::write(root.join(format!("src/fields_{name}.scoop")), source).unwrap();
    }
}

pub(super) fn assert_source_fields(production: &scoop_slib::ValidatedCrossConeSemanticsProduction) {
    use scoop_hir::SourceNominalId;
    use scoop_identity::{
        CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentTypeId,
        SourceDeclarationSite,
    };
    let site = SourceDeclarationSite::new(
        scoop_identity::ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let table = production.hir_interface().nominal_interfaces();
    for (name, kind, arity, count) in [
        ("FieldBase", SourceNominalKind::Class, 0, 1),
        ("FieldBox", SourceNominalKind::Class, 1, 2),
        ("FieldChild", SourceNominalKind::Class, 0, 2),
        ("FieldPrivateOwner", SourceNominalKind::Class, 0, 1),
        ("FieldSingleton", SourceNominalKind::Object, 0, 1),
        ("FieldCombined", SourceNominalKind::Class, 0, 2),
        ("FieldCombinedSingleton", SourceNominalKind::Object, 0, 1),
    ] {
        let key = SourceDeclarationKey::nominal(
            site.clone(),
            CanonicalIdentifier::new(name).unwrap(),
            kind,
            arity,
        );
        let owner = if arity == 0 {
            SourceNominalId::Concrete(PersistentTypeId::from_source_declaration(&key).unwrap())
        } else {
            SourceNominalId::GenericTemplate(
                PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
            )
        };
        let record = table.get(owner).unwrap();
        assert_eq!(
            record.source_shape().declared_fields().len(),
            count,
            "{name}"
        );
        if name == "FieldBox" {
            assert!(
                record
                    .source_shape()
                    .declared_fields()
                    .iter()
                    .all(|field| *field.value_type()
                        == scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 })
            );
            assert!(
                record.members().members().is_empty(),
                "private storage is not public lookup"
            );
        }
    }
}
