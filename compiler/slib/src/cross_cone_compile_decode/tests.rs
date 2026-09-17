use scoop_hir::{
    CanonicalBinderListV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1, CanonicalHirFoundation,
    CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1, NominalInterfaceRecordV1,
    NominalInterfaceSemanticValidationError, NominalSourceShapeV1, PublicMemberRefV1,
    PublicNominalKindV1, SourceNominalId, StructSourceFieldV1, StructSourceShapeV1,
};
use scoop_identity::{
    ArtifactCapabilityProfileId, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    CoreBuiltinNominal, DeclarationScope, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, DefinitionOwnerAtom, DefinitionOwnerChain, FieldIdentityKey,
    NormalizedSourcePath, PackagePath, PersistentConstructorId, PersistentFieldId,
    PersistentFunctionId, PersistentSourceContextId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
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
fn cross_cone_hir_front_validates_a_canonical_nominal_surface() {
    let cone = cone();
    let (foundation, hir_interface, nominal) = nominal_surface(cone.identity());
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
) -> (CanonicalHirFoundation, Vec<u8>, PersistentTypeId) {
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
        top_level_site,
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
            Vec::new(),
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
    (
        foundation,
        interface_with_nominals(vec![record]),
        nominal.id(),
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
    let mut section = CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(records).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    );
    encode(&section.index_for_wire().unwrap()).unwrap()
}
