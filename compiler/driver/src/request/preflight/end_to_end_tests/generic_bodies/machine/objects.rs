use std::collections::BTreeMap;

use object::{Object, ObjectSection, ObjectSymbol};
use scoop_identity::{
    LinkageClass, PersistentCallableBodyId, StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{decode_canonical, encode};

mod associated;
pub(super) use associated::change_associated_atom;
mod finalization;
pub(super) use finalization::CallableFingerprints;
mod directory;
mod member_fingerprints;

pub(super) fn verify(
    emitted: scoop_codegen::EmittedConeObjectSetV2,
    generated: &scoop_codegen::EmittedGeneratedCBridgeObjectSetV1,
    public: &scoop_lir::CrossConeLirBridgeSectionV1,
    imports: &scoop_lir::CanonicalExternalShapeLinkImportsV1,
    dependencies: &scoop_slib::ValidatedCrossConeArtifactClosure,
    expected_bodies: usize,
    core_bodies: &std::collections::BTreeSet<PersistentCallableBodyId>,
) -> BTreeMap<PersistentCallableBodyId, CallableFingerprints> {
    let prepared = crate::object_production::layout::prepare(emitted, generated).unwrap();
    let (callables, safepoints) = prepared.callable_metadata();
    assert_eq!(
        callables.plan().registrations().len(),
        prepared
            .production
            .callable_registrations()
            .registrations()
            .len()
    );
    assert_eq!(
        safepoints.plan().registrations().len(),
        prepared
            .production
            .safepoint_registrations()
            .registrations()
            .len()
    );
    let actual_odr = callables
        .plan()
        .registrations()
        .iter()
        .filter(|entry| {
            !core_bodies.contains(&entry.body())
                && matches!(
                    entry.definition_owner(),
                    RegistrationDefinitionOwner::Odr { .. }
                )
        })
        .count();
    assert_eq!(actual_odr, expected_bodies);
    let closure = prepared.builtins.strong_relocations();
    let owners =
        scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(closure)
            .unwrap();
    let expected_members = prepared
        .production
        .canonical_definitions()
        .plans()
        .iter()
        .filter_map(|record| match record.definition_owner() {
            scoop_identity::ObjectDefinitionPlanOwner::Odr { member } => Some(member),
            scoop_identity::ObjectDefinitionPlanOwner::Strong { .. } => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let actual_members = owners
        .owners()
        .iter()
        .filter_map(|owner| match owner.owner() {
            scoop_slib::LinkDefinitionOwnerV1::OdrDefinition(member) => Some(member),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_members, expected_members);
    let encoded = encode(&owners).unwrap();
    let decoded: scoop_slib::DecodedCanonicalDefinedLinkSymbolOwnerSetV1 =
        decode_canonical(&encoded).unwrap();
    assert_eq!(encode(&decoded).unwrap(), encoded);
    let first = *actual_members.first().unwrap();
    let owner_bytes = encode(&scoop_slib::LinkDefinitionOwnerV1::OdrDefinition(first)).unwrap();
    let mut expected_owner_bytes = vec![0xa2, 0x00, 0x05, 0x01];
    expected_owner_bytes.extend(encode(&first).unwrap());
    assert_eq!(owner_bytes, expected_owner_bytes);
    let requirements = scoop_slib::verify_current_cone_undefined_requirements_v1(
        closure.clone(),
        prepared.production.generated_bridge_plan().clone(),
    )
    .unwrap();
    assert!(requirements.requirements().iter().any(|entry| matches!(
        entry.requirement(),
        scoop_slib::CurrentConeUndefinedRequirementV1::OdrMember { .. }
    )));
    for entry in requirements.requirements() {
        if let scoop_slib::CurrentConeUndefinedRequirementV1::OdrMember { member } =
            entry.requirement()
        {
            assert!(expected_members.contains(&member));
            let mut expected = vec![0xa2, 0x00, 0x09, 0x01];
            expected.extend(encode(&member).unwrap());
            assert_eq!(
                encode(&scoop_slib::FinalUndefinedSymbolRequirementV1::OdrMember { member })
                    .unwrap(),
                expected
            );
        }
    }
    let native = scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        prepared.target_selection.target(),
        &prepared.foundation,
    )
    .unwrap();
    let dependency_owners = dependencies
        .dependency_symbol_owners()
        .cloned()
        .collect::<Vec<_>>();
    let ordinary = scoop_slib::verify_cross_cone_strong_requirements_v1(
        prepared.target_selection.target(),
        closure.clone(),
        &dependency_owners,
        public,
    )
    .unwrap();
    let shape =
        scoop_slib::verify_external_shape_requirements_v1(&ordinary, closure.producer(), imports)
            .unwrap();
    let undefined =
        crate::object_production::layout::complete_requirements(&prepared, &native, &shape)
            .unwrap();
    let cone = scoop_slib::ConeRecord::new(
        prepared.production.image_plan().cone().coordinate().clone(),
        scoop_slib::ConeKind::Library,
        scoop_slib::ConeSourceForm::Manifest,
    )
    .unwrap();
    let finalized = prepared.finalize(&undefined, &cone, Vec::new()).unwrap();
    let objects = finalized.projection.link_objects().final_objects();
    let canonical = finalized
        .projection
        .strong_production()
        .canonical_callable_definitions();
    if expected_bodies == 1 {
        member_fingerprints::check_inputs(objects, &finalized.foundation, canonical);
    }
    let fingerprints = finalization::check(objects, canonical);
    let code = scoop_slib::compute_cross_cone_layout_code_fingerprint_v1(
        finalized.projection,
        native,
        owners,
        undefined,
        &shape,
        scoop_slib::LirLinkSupportSectionV1::default(),
    )
    .unwrap();
    directory::check(code.code(), &expected_members);
    fingerprints
}

pub(super) fn check(
    emitted: &scoop_codegen::EmittedConeObjectSetV2,
) -> BTreeMap<PersistentCallableBodyId, Vec<u8>> {
    let production = &emitted.production;
    let normalization = emitted.target().contract().native_symbol_normalization();
    let mut bodies = BTreeMap::new();
    for member in &emitted.members {
        let bytes = member.bytes.clone();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        let envelope =
            scoop_slib::validate_scoop_lir_llvm_22_1_object_envelope_v1(emitted.target(), &bytes)
                .unwrap();
        let check_linkage = |request: scoop_identity::PersistentSymbolRequest| {
            let name = normalization.compiler_generated_object_symbol(request.symbol().as_str());
            let symbol = envelope
                .sections()
                .envelope()
                .symbols()
                .iter()
                .find(|symbol| symbol.name() == name.as_bytes())
                .unwrap();
            let expected = if request.linkage() == LinkageClass::OdrWeak {
                scoop_slib::ObjectSymbolKindV1::ExternalWeakDefinition
            } else {
                scoop_slib::ObjectSymbolKindV1::ExternalStrongDefinition
            };
            assert_eq!(symbol.kind(), expected, "{name}");
        };
        for definition in member.units.definition_plans() {
            let plan = production
                .canonical_definitions()
                .plan(*definition)
                .unwrap();
            let symbol_name = |request: scoop_identity::PersistentSymbolRequest| {
                normalization.compiler_generated_object_symbol(request.symbol().as_str())
            };
            let primary = object
                .symbol_by_name(&symbol_name(plan.primary_symbol()))
                .unwrap();
            let weak = plan.primary_symbol().linkage() == LinkageClass::OdrWeak;
            assert!(primary.is_definition());
            check_linkage(plan.primary_symbol());
            for boundary in plan.atom_boundaries() {
                let start = object
                    .symbol_by_name(&symbol_name(boundary.start()))
                    .unwrap();
                let end = object.symbol_by_name(&symbol_name(boundary.end())).unwrap();
                assert!(start.is_definition() && end.is_definition());
                check_linkage(boundary.start());
                check_linkage(boundary.end());
                assert_eq!(start.section_index(), end.section_index());
                assert!(start.address() < end.address());
                if boundary.atom() == plan.primary_atom() {
                    assert_eq!(start.address(), primary.address());
                    assert_eq!(start.section_index(), primary.section_index());
                }
            }
            let section = object
                .section_by_index(primary.section_index().unwrap())
                .unwrap();
            let offset = usize::try_from(primary.address() - section.address()).unwrap();
            let data = &section.data().unwrap()[offset..];
            match (plan.definition_role(), plan.owner().kind()) {
                (
                    StrongDefinitionRole::CallableBody,
                    StrongDefinitionEntityKind::CallableBody(body),
                ) if weak => {
                    assert!(bodies.insert(body, bytes.clone()).is_none());
                }
                (
                    StrongDefinitionRole::CallableRegistration,
                    StrongDefinitionEntityKind::CallableBody(body),
                ) => {
                    let registration = production
                        .registration_production()
                        .callables()
                        .registrations()
                        .iter()
                        .find(|registration| registration.body() == body)
                        .unwrap();
                    check_registration(data, body.as_array(), registration.definition_owner(), 144);
                }
                (
                    StrongDefinitionRole::SafepointRegistration,
                    StrongDefinitionEntityKind::SafepointSite(site),
                ) => {
                    let registration = production
                        .registration_production()
                        .safepoints()
                        .registrations()
                        .iter()
                        .find(|registration| registration.site() == site)
                        .unwrap();
                    check_registration(data, site.as_array(), registration.definition_owner(), 168);
                }
                _ => {}
            }
        }
    }
    bodies
}

fn check_registration(
    data: &[u8],
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
    size: u32,
) {
    assert_eq!(
        &data[8..12],
        &scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1.to_le_bytes()
    );
    assert_eq!(&data[12..16], &size.to_le_bytes());
    assert_eq!(&data[20..24], &[0; 4]);
    assert_eq!(&data[24..56], semantic);
    match owner {
        RegistrationDefinitionOwner::Strong => {
            assert_eq!(&data[16..20], &1_u32.to_le_bytes());
            assert_eq!(&data[56..120], &[0; 64]);
        }
        RegistrationDefinitionOwner::Odr { group, member } => {
            assert_eq!(&data[16..20], &2_u32.to_le_bytes());
            assert_eq!(&data[56..88], group.as_array());
            assert_eq!(&data[88..120], member.as_array());
        }
    }
}
