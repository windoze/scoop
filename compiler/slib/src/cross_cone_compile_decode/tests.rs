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
    NominalInterfaceSemanticValidationError, NominalSourceShapeV1,
    PropertyAccessorClosureValidationError, PropertyCapabilityV1, PropertyInterfaceRecordV1,
    PropertyInterfaceSemanticValidationError, PropertyInterfaceSetSemanticValidationError,
    PropertyPublicAccessV1, PropertyRepresentationV1, PropertySetterPublicAccessV1,
    PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicMemberRefV1, PublicNominalKindV1,
    SourceNominalId, SourceParameterShapeV1, StructSourceFieldV1, StructSourceShapeV1,
    TypeParameterBinderV1, TypeParameterBoundsV1,
};
use scoop_identity::{
    AccessorRole, ArtifactCapabilityProfileId, CallableTemplateOrigin, CanonicalIdentifier,
    CborIdentityRecord, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin,
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
    DependencyRecord, MemberPurposeSet, MetadataLocation, SectionLocation,
    hir_cross_cone_interface_capability, lir_cross_cone_link_closure_capability,
    lir_cross_cone_param_free_bridge_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, mir_cross_cone_param_free_bridge_capability,
    strong_compile_decode::tests::{
        build_artifact_for_profile, build_artifact_for_profile_with_dependencies, cone, open_graph,
        required_sections, section,
    },
};

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
    let _ = front.lir_foundation_wire();
    let _ = front.lir_strong_production_wire();
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
    assert!(matches!(
        validated.hir_core_production().core_interface(),
        scoop_hir::CoreHirInterfaceBranchV1::NotCore
    ));
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
        CrossConeHirInternalClosureValidationError::PropertyAccessors(
            PropertyAccessorClosureValidationError::MissingPublicAccessor { .. },
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
        NominalInterfaceRecordV1::try_new(
            SourceNominalId::Concrete(unit),
            PublicNominalKindV1::Struct,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Struct(StructSourceShapeV1::try_new(Vec::new()).unwrap()),
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

    let Err(CrossConeHirNominalSurfaceError::NominalInterfaces(error)) =
        front.validate_nominal_surface(Vec::new())
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
    retarget_lir_sections(&cone, &mut lir);
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
    retarget_lir_sections(&cone, &mut lir);
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

fn retarget_lir_sections(cone: &ConeRecord, lir: &mut [crate::MetadataSection]) {
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

    let (foundation, production) =
        crate::link_decode::strong_production_fixture_for_test(cone.coordinate().clone());
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
        vec![0x80],
    ));
    lir.push(section(
        MetadataLocation::Lir,
        lir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0x80],
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
        0xaa, 0x01, 0x80, 0x02, 0x80, 0x03, 0x80, 0x04, 0x80, 0x05, 0x80, 0x06, 0x80, 0x07, 0x80,
        0x08, 0x80, 0x09, 0x80, 0x0a, 0x80,
    ]
}

fn nominal_surface(
    cone: scoop_identity::ConeIdentity,
    matching_callable_parameter: bool,
    matching_property_owner: bool,
    include_public_accessors: bool,
) -> (
    CanonicalHirFoundation,
    Vec<u8>,
    PersistentTypeId,
    PersistentFunctionId,
    PersistentConstructorId,
    PersistentPropertyId,
    PersistentExtensionPropertyId,
    PersistentPropertyAccessorId,
) {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/Value.scoop").unwrap()).unwrap();
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let context =
        CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone()).unwrap();
    let origin =
        DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &context_key)
            .unwrap();
    let top_level_site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::nominal(
        top_level_site.clone(),
        CanonicalIdentifier::new("Value").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let nominal = CborIdentityRecord::<PersistentTypeId, _>::from_key(declaration).unwrap();
    let owned_site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(nominal.id())]),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let constructor = CborIdentityRecord::<PersistentConstructorId, _>::from_key(
        SourceDeclarationKey::constructor(owned_site.clone(), Vec::new()),
    )
    .unwrap();
    let member =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            owned_site,
            CanonicalIdentifier::new("size").unwrap(),
            0,
            None,
            vec![SignatureTypeKey::Nominal(nominal.id())],
        ))
        .unwrap();
    let extension = CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                cone,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("mapValue").unwrap(),
            1,
            Some(SignatureTypeKey::Nominal(nominal.id())),
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        ),
    )
    .unwrap();
    let property =
        CborIdentityRecord::<PersistentPropertyId, _>::from_key(SourceDeclarationKey::property(
            top_level_site.clone(),
            CanonicalIdentifier::new("current").unwrap(),
        ))
        .unwrap();
    let getter = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
        PropertyAccessorKey::new(PropertyOwner::Property(property.id()), AccessorRole::Getter),
    )
    .unwrap();
    let extension_property = CborIdentityRecord::<PersistentExtensionPropertyId, _>::from_key(
        SourceDeclarationKey::extension_property(
            top_level_site,
            CanonicalIdentifier::new("mapped").unwrap(),
            1,
            SignatureTypeKey::Nominal(nominal.id()),
        ),
    )
    .unwrap();
    let extension_getter =
        CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(PropertyAccessorKey::new(
            PropertyOwner::ExtensionProperty(extension_property.id()),
            AccessorRole::Getter,
        ))
        .unwrap();
    let extension_setter =
        CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(PropertyAccessorKey::new(
            PropertyOwner::ExtensionProperty(extension_property.id()),
            AccessorRole::Setter,
        ))
        .unwrap();
    let field = CborIdentityRecord::<PersistentFieldId, _>::from_key(
        FieldIdentityKey::source_declared(
            nominal.key(),
            CanonicalIdentifier::new("payload").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let mut foundation = base_hir_foundation();
    foundation
        .set_sources(vec![
            scoop_hir::SourceRecord::from_utf8(source, "class Value", [0, 5]).unwrap(),
        ])
        .unwrap();
    foundation.set_source_contexts(vec![context]).unwrap();
    foundation
        .set_definition_origins(vec![
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(nominal.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Constructor(constructor.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Function(member.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::GenericFunction(extension.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Property(property.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(getter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::ExtensionProperty(extension_property.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(extension_getter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(extension_setter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(DefinitionOriginSubject::Field(field.id()), origin),
        ])
        .unwrap();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            nominal.clone(),
        ])
        .unwrap();
    foundation
        .set_constructors(vec![constructor.clone()])
        .unwrap();
    foundation.set_functions(vec![member.clone()]).unwrap();
    foundation
        .set_generic_functions(vec![extension.clone()])
        .unwrap();
    foundation.set_properties(vec![property.clone()]).unwrap();
    foundation
        .set_extension_properties(vec![extension_property.clone()])
        .unwrap();
    foundation
        .set_property_accessors(vec![
            getter.clone(),
            extension_getter.clone(),
            extension_setter.clone(),
        ])
        .unwrap();
    foundation.set_fields(vec![field.clone()]).unwrap();

    let record = NominalInterfaceRecordV1::try_new(
        SourceNominalId::Concrete(nominal.id()),
        PublicNominalKindV1::Struct,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![constructor.id()]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![PublicMemberRefV1::Callable(
            CallableTemplateOrigin::Function(member.id()),
        )])
        .unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(vec![StructSourceFieldV1::new(
                field.id(),
                SignatureTypeKey::Nominal(nominal.id()),
            )])
            .unwrap(),
        ),
    )
    .unwrap();
    let callable_parameter = if matching_callable_parameter {
        SignatureTypeKey::Nominal(nominal.id())
    } else {
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    };
    let callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(member.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("value").unwrap(),
            callable_parameter,
        )])
        .unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let extension_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::GenericFunction(extension.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("mapped").unwrap(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        )])
        .unwrap(),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let constructor_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Constructor(constructor.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let property_owner = if matching_property_owner {
        PublicDeclarationOwnerV1::TopLevel
    } else {
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id()))
    };
    let getter_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Accessor(getter.id()),
        property_owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let extension_getter_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Accessor(extension_getter.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    let property_record = PropertyInterfaceRecordV1::try_new(
        PropertyOwner::Property(property.id()),
        property_owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        SignatureTypeKey::Nominal(nominal.id()),
        PropertyCapabilityV1::read_only(getter.id()),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    )
    .unwrap();
    let extension_property_record = PropertyInterfaceRecordV1::try_new(
        PropertyOwner::ExtensionProperty(extension_property.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        PropertyCapabilityV1::try_read_write(
            extension_getter.id(),
            extension_setter.id(),
            PropertySetterPublicAccessV1::Restricted,
        )
        .unwrap(),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    )
    .unwrap();
    let mut callables = vec![callable, constructor_callable, extension_callable];
    if include_public_accessors {
        callables.push(getter_callable);
        callables.push(extension_getter_callable);
    }
    (
        foundation,
        interface_with_declarations(
            vec![record],
            callables,
            vec![property_record, extension_property_record],
        ),
        nominal.id(),
        member.id(),
        constructor.id(),
        property.id(),
        extension_property.id(),
        getter.id(),
    )
}

fn enum_variant_callable_surface(
    cone: scoop_identity::ConeIdentity,
) -> (CanonicalHirFoundation, Vec<u8>, PersistentEnumVariantId) {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/Choice.scoop").unwrap()).unwrap();
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let context =
        CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone()).unwrap();
    let origin =
        DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 6).unwrap(), &context_key)
            .unwrap();
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Choice").unwrap(),
        SourceNominalKind::Enum,
        0,
    );
    let nominal = CborIdentityRecord::<PersistentTypeId, _>::from_key(declaration).unwrap();
    let variant_key =
        EnumVariantIdentityKey::source(nominal.key(), CanonicalIdentifier::new("Only").unwrap())
            .unwrap();
    let variant = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(variant_key).unwrap();

    let mut foundation = base_hir_foundation();
    foundation
        .set_sources(vec![
            scoop_hir::SourceRecord::from_utf8(source, "enum Choice", [0, 6]).unwrap(),
        ])
        .unwrap();
    foundation.set_source_contexts(vec![context]).unwrap();
    foundation
        .set_definition_origins(vec![
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(nominal.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(DefinitionOriginSubject::EnumVariant(variant.id()), origin),
        ])
        .unwrap();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            nominal.clone(),
        ])
        .unwrap();
    foundation.set_enum_variants(vec![variant.clone()]).unwrap();

    let nominal_record = NominalInterfaceRecordV1::try_new(
        SourceNominalId::Concrete(nominal.id()),
        PublicNominalKindV1::Enum,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Enum(
            EnumSourceShapeV1::try_new(vec![
                EnumSourceVariantV1::try_new(
                    variant.id(),
                    EnumSourceVariantStyleV1::Unit,
                    Vec::new(),
                )
                .unwrap(),
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::VariantConstructor(variant.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap();
    (
        foundation,
        interface_with_declarations(vec![nominal_record], vec![callable], Vec::new()),
        variant.id(),
    )
}

fn base_hir_foundation() -> CanonicalHirFoundation {
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    foundation
}

fn interface_with_nominals(records: Vec<NominalInterfaceRecordV1>) -> Vec<u8> {
    interface_with_declarations(records, Vec::new(), Vec::new())
}

fn interface_with_declarations(
    nominals: Vec<NominalInterfaceRecordV1>,
    callables: Vec<CallableInterfaceRecordV1>,
    properties: Vec<PropertyInterfaceRecordV1>,
) -> Vec<u8> {
    let mut section = CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
        CanonicalCallableInterfacesV1::try_new(callables).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(properties).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    );
    encode(&section.index_for_wire().unwrap()).unwrap()
}

fn scoop_effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}
