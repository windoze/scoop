use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, ExportBindingKey, PackagePath, SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{
    CanonicalReexportRoutesV1, PublicExportBindingRecordV1, ReexportRouteHopV1, ReexportRouteV1,
};

#[test]
fn core_and_ordinary_shapes_use_the_same_local_public_binding_projection() {
    for current in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let concrete =
            CborIdentityRecord::<PersistentTypeId, _>::from_key(nominal(current, "Record", 0))
                .unwrap();
        let generic = nominal(current, "Container", 1);
        let alias = SourceDeclarationKey::type_alias(site(current), name("Alias"));
        let concrete_binding = binding(
            current,
            "Record",
            BindingTarget::type_name(concrete.key()).unwrap(),
        );
        let generic_binding = binding(
            current,
            "Container",
            BindingTarget::type_name(&generic).unwrap(),
        );
        let alias_binding = binding(current, "Alias", BindingTarget::type_alias(&alias).unwrap());
        let foreign = if current == ConeIdentity::CORE {
            ConeIdentity::SINGLE_FILE
        } else {
            ConeIdentity::CORE
        };
        let foreign_type = nominal(foreign, "Foreign", 0);
        let original = binding(
            foreign,
            "Foreign",
            BindingTarget::type_name(&foreign_type).unwrap(),
        );
        let forwarded = binding(
            current,
            "Foreign",
            BindingTarget::type_name(&foreign_type).unwrap(),
        );
        let mut records = [&concrete_binding, &generic_binding, &alias_binding]
            .into_iter()
            .map(|binding| {
                PublicExportBindingRecordV1::new(
                    binding.id(),
                    ExportBindingSourceV1::DeclaredCurrent {
                        declaration: binding.key().target(),
                    },
                )
            })
            .collect::<Vec<_>>();
        records.push(PublicExportBindingRecordV1::new(
            forwarded.id(),
            ExportBindingSourceV1::Reexport {
                routes: CanonicalReexportRoutesV1::try_new(vec![
                    ReexportRouteV1::try_new(
                        foreign,
                        vec![ReexportRouteHopV1::new(foreign, original.id())],
                    )
                    .unwrap(),
                ])
                .unwrap(),
            },
        ));
        let public = CanonicalPublicExportBindingsV1::try_new(records).unwrap();
        let direct = CanonicalDirectPublicSurfaceV1::from_public_bindings(&public).unwrap();
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(vec![concrete.clone()]).unwrap();
        foundation
            .set_export_bindings(vec![
                concrete_binding,
                generic_binding,
                alias_binding,
                forwarded,
            ])
            .unwrap();
        let projected = PublicNominalShapeRequirementsV1::from_public_bindings(&public).unwrap();
        let decoded_projection =
            PublicNominalShapeRequirementsV1::from_direct_surface(&direct, &foundation).unwrap();
        assert_eq!(projected, decoded_projection);
        assert_eq!(projected.roots().len(), 1);
        assert_eq!(projected.roots()[0].source(), concrete.id());
        assert_eq!(
            projected.roots()[0].exact(),
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(concrete.id())).unwrap()
        );
        assert_eq!(
            projected.source_declarations(&foundation).unwrap(),
            vec![concrete.key().clone()]
        );
    }
}

#[test]
fn missing_shared_bindings_and_local_nominals_are_rejected() {
    let source = nominal(ConeIdentity::SINGLE_FILE, "Record", 0);
    let binding = binding(
        ConeIdentity::SINGLE_FILE,
        "Record",
        BindingTarget::type_name(&source).unwrap(),
    );
    let direct = CanonicalDirectPublicSurfaceV1::try_new(vec![binding.id()]).unwrap();
    let mut foundation = CanonicalHirFoundation::empty();
    assert_eq!(
        PublicNominalShapeRequirementsV1::from_direct_surface(&direct, &foundation),
        Err(PublicNominalShapeProjectionError::MissingBinding(
            binding.id()
        )),
    );
    foundation.set_export_bindings(vec![binding]).unwrap();
    assert_eq!(
        PublicNominalShapeRequirementsV1::from_direct_surface(&direct, &foundation),
        Err(PublicNominalShapeProjectionError::MissingSourceNominal(
            PersistentTypeId::from_source_declaration(&source).unwrap()
        )),
    );
}

fn nominal(cone: ConeIdentity, text: &str, parameters: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(cone),
        name(text),
        SourceNominalKind::Struct,
        parameters,
    )
}

fn site(cone: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn name(text: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(text).unwrap()
}

fn binding(
    cone: ConeIdentity,
    text: &str,
    target: BindingTarget,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        cone,
        PackagePath::root(),
        name(text),
        target,
    ))
    .unwrap()
}
