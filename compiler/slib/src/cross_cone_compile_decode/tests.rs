use scoop_hir::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1,
    CallableInterfaceSemanticValidationError, CallableInterfaceSetSemanticValidationError,
    CallableModalityV1, CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1,
    CanonicalBinderListV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1, CanonicalHirFoundation,
    CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1,
    CanonicalSourceParameterShapesV1, CanonicalTypeAliasInterfacesV1,
    CrossConeHirInterfaceSectionV1, CrossConeHirInternalClosureValidationError, EnumSourceShapeV1,
    EnumSourceVariantStyleV1, EnumSourceVariantV1, NominalInterfaceRecordV1,
    NominalInterfaceSemanticValidationError, NominalInterfaceSetSemanticValidationError,
    NominalSourceFieldV1, NominalSourceShapeV1, PropertyAccessorClosureValidationError,
    PropertyInterfaceRecordV1, PropertyInterfaceSemanticValidationError,
    PropertyInterfaceSetSemanticValidationError, PropertyPublicAccessV1, PropertyRepresentationV1,
    PropertySetterPublicAccessV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
    PublicMemberRefV1, PublicNominalKindV1, SourceNominalId, SourceParameterShapeV1,
    StructSourceShapeV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};
use scoop_identity::{
    AccessorRole, ArtifactCapabilityProfileId, CallableTemplateOrigin, CanonicalIdentifier,
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin,
    DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerAtom, DefinitionOwnerChain,
    Effect, EnumVariantIdentityKey, FieldIdentityKey, GcEffect, NormalizedSourcePath, PackagePath,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExtensionPropertyId,
    PersistentFieldId, PersistentFunctionId, PersistentGenericFunctionId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::encode;

use super::*;
use crate::{
    ArtifactCapabilityProfile, ArtifactProfileInventoryError, ArtifactProfileView, ConeRecord,
    CrossConeHirNominalAuthorityError, DependencyRecord, MemberPurposeSet, MetadataLocation,
    SectionLocation, hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_identity_foundation_capability, lir_cross_cone_link_closure_capability,
    lir_cross_cone_param_free_bridge_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, mir_cross_cone_param_free_bridge_capability,
    strong_compile_decode::tests::{
        build_artifact_for_profile, build_artifact_for_profile_with_dependencies, cone, open_graph,
        required_sections, section,
    },
};

mod callable_declarations;
mod const_value;
mod intrinsics;
mod lir_bridge;
mod mir_bridge;
mod nominal_declarations;
mod nominal_fields;
mod property_declarations;
mod source_interface;
mod type_alias;

#[test]
fn cross_cone_profile_decodes_the_complete_hir_front() {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let front = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();

    assert_eq!(front.coordinate(), cone().coordinate());
    assert_eq!(front.identity(), cone().identity());
    let _ = front.hir_foundation_wire();
    let _ = front.hir_core_production_wire();
    let _ = front.hir_interface_wire();
    let _ = front.mir_foundation_wire();
    let _ = front.mir_core_production_wire();
    let _ = front.mir_cross_cone_bridge_wire();
    let _ = front.lir_foundation_wire();
    let _ = front.lir_strong_production_wire();
    let _ = front.lir_cross_cone_bridge_wire();
}

#[test]
fn cross_cone_hir_front_rejects_the_legacy_profile_before_payloads() {
    let (hir, mir, lir) = required_sections();
    let bytes = build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir,
        mir,
        lir,
        false,
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::WrongProfile {
            expected,
            actual,
        }) if expected == ArtifactCapabilityProfileId::cross_cone_semantics_strong()
            && actual == ArtifactCapabilityProfileId::single_cone_strong()
    ));
}

#[test]
fn cross_cone_hir_front_requires_the_general_hir_capability() {
    let (hir, mut mir, mut lir) = required_sections();
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    let bytes = build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        hir,
        mir,
        lir,
        false,
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                view: ArtifactProfileView::Compile,
                location: SectionLocation::Hir,
                capability,
            }
        )) if capability == hir_cross_cone_interface_capability()
    ));
}

#[test]
fn cross_cone_hir_front_rejects_noncanonical_general_hir_payload() {
    let bytes = cross_cone_artifact(vec![0x80]);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::InnerSection {
            location: MetadataLocation::Hir,
            capability,
            ..
        }) if capability == hir_cross_cone_interface_capability()
    ));
}

#[test]
fn cross_cone_hir_front_validates_the_legacy_direct_surface() {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap();

    assert_eq!(validated.identity(), cone().identity());
    assert!(
        validated
            .hir_core_production()
            .direct_public_surface()
            .bindings()
            .is_empty()
    );
    assert!(
        validated
            .hir_core_production()
            .compiler_protocol_definitions()
            .is_none()
    );
}

#[test]
fn cross_cone_hir_front_rejects_an_open_property_accessor_closure() {
    let cone = cone();
    let (foundation, hir_interface, _, _, _, _, _, _) =
        nominal_surface(cone.identity(), true, true, false);
    let bytes =
        cross_cone_artifact_for_with_hir_foundation(cone, Vec::new(), &foundation, hir_interface);
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap();

    let Err(CrossConeHirInternalClosureError::Interface(
        CrossConeHirInternalClosureValidationError::CallableDeclarations(
            scoop_hir::CallableDeclarationInventoryError::Missing(
                CallableTemplateOrigin::Accessor(_),
            ),
        ),
    )) = front.validate_internal_hir_closures()
    else {
        panic!("an open property accessor closure must fail before surface validation");
    };
}

#[test]
fn cross_cone_hir_front_validates_a_canonical_nominal_surface() {
    let cone = cone();
    let (foundation, hir_interface, nominal, _, _, _, _, _) =
        nominal_surface(cone.identity(), true, true, true);
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        cone.clone(),
        Vec::new(),
        &foundation,
        hir_interface,
    );
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap();

    assert_eq!(validated.identity(), cone.identity());
    assert!(
        validated
            .hir_interface()
            .nominal_interfaces()
            .get(SourceNominalId::Concrete(nominal))
            .is_some()
    );
}

#[test]
fn cross_cone_hir_front_validates_a_canonical_property_surface() {
    let cone = cone();
    let (foundation, hir_interface, _, _, _, property, extension_property, _) =
        nominal_surface(cone.identity(), true, true, true);
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        cone.clone(),
        Vec::new(),
        &foundation,
        hir_interface,
    );
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap();

    assert!(
        validated
            .hir_interface()
            .property_interfaces()
            .get(PropertyOwner::Property(property))
            .is_some()
    );
    assert!(
        validated
            .hir_interface()
            .property_interfaces()
            .get(PropertyOwner::ExtensionProperty(extension_property))
            .is_some()
    );
}

#[test]
fn cross_cone_hir_front_validates_a_canonical_callable_surface() {
    let builtin_bytes = builtin_provider_artifact();
    let builtin = nominal_fields::front(&builtin_bytes)
        .validate_nominal_surface(vec![])
        .unwrap();
    let cone = cone();
    let (foundation, hir_interface, _, callable, constructor, _, _, getter) =
        nominal_surface(cone.identity(), true, true, true);
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        cone.clone(),
        Vec::new(),
        &foundation,
        hir_interface,
    );
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(vec![builtin.nominal_provider_view()])
        .unwrap();

    assert!(
        validated
            .hir_interface()
            .callable_interfaces()
            .get(CallableTemplateOrigin::Function(callable))
            .is_some()
    );
    assert!(
        validated
            .hir_interface()
            .callable_interfaces()
            .get(CallableTemplateOrigin::Constructor(constructor))
            .is_some()
    );
    assert!(
        validated
            .hir_interface()
            .callable_interfaces()
            .get(CallableTemplateOrigin::Accessor(getter))
            .is_some()
    );
    assert!(
        validated
            .hir_interface()
            .callable_interfaces()
            .records()
            .iter()
            .any(|record| matches!(
                record.declaration(),
                CallableTemplateOrigin::GenericFunction(_)
            ))
    );
}

#[test]
fn cross_cone_hir_front_validates_an_enum_variant_constructor_surface() {
    let cone = cone();
    let (foundation, hir_interface, variant) = enum_variant_callable_surface(cone.identity());
    let bytes =
        cross_cone_artifact_for_with_hir_foundation(cone, Vec::new(), &foundation, hir_interface);
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let validated = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap()
        .validate_callable_surface(Vec::new())
        .unwrap();

    assert!(
        validated
            .hir_interface()
            .callable_interfaces()
            .get(CallableTemplateOrigin::VariantConstructor(variant))
            .is_some()
    );
}

#[test]
fn cross_cone_hir_front_rejects_a_callable_parameter_identity_mismatch() {
    let cone = cone();
    let (foundation, hir_interface, nominal, _, _, _, _, _) =
        nominal_surface(cone.identity(), false, true, true);
    let bytes =
        cross_cone_artifact_for_with_hir_foundation(cone, Vec::new(), &foundation, hir_interface);
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap()
        .validate_property_surface(Vec::new())
        .unwrap();

    let Err(CrossConeHirCallableSurfaceError::CallableInterfaces(error)) =
        front.validate_callable_surface(Vec::new())
    else {
        panic!("a callable parameter mismatch must fail semantic validation");
    };
    assert!(matches!(
        error.as_ref(),
        CallableInterfaceSetSemanticValidationError::Record {
            error: CallableInterfaceSemanticValidationError::ParameterTypeMismatch {
                expected,
                actual,
                ..
            },
            ..
        } if expected.as_ref() == &SignatureTypeKey::Nominal(nominal)
            && actual.as_ref()
                == &SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    ));
}

#[test]
fn cross_cone_hir_front_rejects_a_property_owner_identity_mismatch() {
    let cone = cone();
    let (foundation, hir_interface, nominal, _, _, _, _, _) =
        nominal_surface(cone.identity(), true, false, true);
    let bytes =
        cross_cone_artifact_for_with_hir_foundation(cone, Vec::new(), &foundation, hir_interface);
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
        .unwrap();

    let Err(CrossConeHirPropertySurfaceError::PropertyInterfaces(error)) =
        front.validate_property_surface(Vec::new())
    else {
        panic!("a property owner mismatch must fail semantic validation");
    };
    assert!(matches!(
        error.as_ref(),
        PropertyInterfaceSetSemanticValidationError::Record {
            error: PropertyInterfaceSemanticValidationError::Owner {
                expected: PublicDeclarationOwnerV1::TopLevel,
                actual: PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(actual)),
            },
            ..
        } if *actual == nominal
    ));
}

#[test]
fn cross_cone_hir_front_rejects_a_foreign_nominal_claim() {
    let cone = cone();
    let foundation = base_hir_foundation();
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let interface = interface_with_nominals(vec![
        crate::nominal_interface_fixture::public_record(
            SourceNominalId::Concrete(unit),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Struct(
                StructSourceShapeV1::try_new(
                    Vec::new(),
                    scoop_hir::NominalCLayoutPolicyV1::Ordinary,
                    false,
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    ]);
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        cone.clone(),
        Vec::new(),
        &foundation,
        interface,
    );
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    let front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap();

    let Err(CrossConeHirNominalSurfaceError::NominalInterfaces(error)) = front
        .validate_definition_sources(&[])
        .unwrap()
        .validate_nominal_surface(Vec::new())
    else {
        panic!("a foreign nominal claim must fail semantic validation");
    };
    assert!(matches!(
        error.as_ref(),
        NominalInterfaceSetSemanticValidationError::Record {
            error: NominalInterfaceSemanticValidationError::Declaration(
                CrossConeHirNominalAuthorityError::ForeignDeclaration {
                    expected,
                    actual: ConeIdentity::CORE,
                    ..
                }
            ),
            ..
        } if *expected == cone.identity()
    ));
}

pub(crate) fn cross_cone_artifact(hir_interface: Vec<u8>) -> Vec<u8> {
    cross_cone_artifact_for(cone(), Vec::new(), hir_interface)
}

pub(crate) fn cross_cone_artifact_for(
    cone: ConeRecord,
    dependencies: Vec<DependencyRecord>,
    hir_interface: Vec<u8>,
) -> Vec<u8> {
    let (mut hir, mut mir, mut lir) = required_sections();
    retarget_lir_sections(&cone, &dependencies, &mut lir);
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        hir_interface,
    ));
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    build_artifact_for_profile_with_dependencies(
        cone,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        dependencies,
        hir,
        mir,
        lir,
        false,
    )
}

pub(crate) fn cross_cone_artifact_for_with_hir_foundation(
    cone: ConeRecord,
    dependencies: Vec<DependencyRecord>,
    hir_foundation: &CanonicalHirFoundation,
    hir_interface: Vec<u8>,
) -> Vec<u8> {
    let (mut hir, mut mir, mut lir) = required_sections();
    retarget_lir_sections(&cone, &dependencies, &mut lir);
    let foundation = hir
        .iter_mut()
        .find(|section| section.capability() == &hir_identity_foundation_capability())
        .expect("the shared fixture has a HIR identity foundation");
    *foundation = section(
        MetadataLocation::Hir,
        hir_identity_foundation_capability(),
        MemberPurposeSet::COMPILE,
        encode(hir_foundation).unwrap(),
    );
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        hir_interface,
    ));
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    build_artifact_for_profile_with_dependencies(
        cone,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        dependencies,
        hir,
        mir,
        lir,
        false,
    )
}

fn retarget_lir_sections(
    cone: &ConeRecord,
    dependencies: &[DependencyRecord],
    lir: &mut [crate::MetadataSection],
) {
    if cone.identity() == scoop_identity::ConeIdentity::CORE {
        let foundation_section = lir
            .iter_mut()
            .find(|section| section.capability() == &lir_identity_foundation_capability())
            .expect("the shared fixture has a LIR identity foundation");
        *foundation_section = section(
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&scoop_lir::CanonicalLirFoundation::empty()).unwrap(),
        );
        return;
    }

    let (foundation, production) = crate::link_decode::strong_production_fixture_for_test(
        cone.coordinate().clone(),
        &dependencies
            .iter()
            .map(DependencyRecord::identity)
            .collect::<Vec<_>>(),
    );
    let foundation_section = lir
        .iter_mut()
        .find(|section| section.capability() == &lir_identity_foundation_capability())
        .expect("the shared fixture has a LIR identity foundation");
    *foundation_section = section(
        MetadataLocation::Lir,
        lir_identity_foundation_capability(),
        MemberPurposeSet::COMPILE,
        encode(&foundation).unwrap(),
    );
    let production_section = lir
        .iter_mut()
        .find(|section| section.capability() == &lir_strong_production_capability())
        .expect("the shared fixture has a LIR strong-production section");
    *production_section = section(
        MetadataLocation::Lir,
        lir_strong_production_capability(),
        MemberPurposeSet::COMPILE_AND_LINK,
        encode(&production).unwrap(),
    );
}

fn add_cross_cone_bridge_sections(
    mir: &mut Vec<crate::MetadataSection>,
    lir: &mut Vec<crate::MetadataSection>,
) {
    mir.push(section(
        MetadataLocation::Mir,
        mir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0xa2, 0x01, 0x80, 0x02, 0x80],
    ));
    lir.push(section(
        MetadataLocation::Lir,
        lir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0xa2, 0x01, 0x80, 0x02, 0x80],
    ));
    lir.push(section(
        MetadataLocation::Lir,
        lir_cross_cone_link_closure_capability(),
        MemberPurposeSet::LINK,
        vec![0x80],
    ));
}

pub(crate) fn empty_cross_cone_hir_interface() -> Vec<u8> {
    vec![
        0xaa, 0x01, 0x80, 0x02, 0xa2, 0x01, 0x80, 0x02, 0x80, 0x03, 0xa2, 0x01, 0x80, 0x02, 0x80,
        0x04, 0xa2, 0x01, 0x80, 0x02, 0x80, 0x05, 0x80, 0x06, 0x80, 0x07, 0x80, 0x08, 0x80, 0x09,
        0x80, 0x0a, 0x80,
    ]
}

mod surface_fixture;
use surface_fixture::*;
