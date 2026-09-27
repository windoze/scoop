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
mod member_fingerprints;

pub(super) fn verify(
    emitted: scoop_codegen::EmittedConeObjectSetV2,
    generated: &scoop_codegen::EmittedGeneratedCBridgeObjectSetV1,
    public: &scoop_lir::CrossConeLirBridgeSectionV1,
    imports: &scoop_lir::CanonicalExternalShapeLinkImportsV1,
    dependencies: &[scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1],
    expected_bodies: usize,
) -> BTreeMap<PersistentCallableBodyId, CallableFingerprints> {
    let prepared = crate::object_production::layout::prepare(emitted, generated).unwrap();
    let (callables, safepoints) = prepared.verify_callable_metadata().unwrap();
    assert_eq!(
        callables.registrations().len(),
        prepared
            .production
            .callable_registrations()
            .registrations()
            .len()
    );
    assert_eq!(
        safepoints.registrations().len(),
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
            matches!(
                entry.definition_owner(),
                RegistrationDefinitionOwner::Odr { .. }
            )
        })
        .count();
    assert_eq!(actual_odr, expected_bodies);
    let closure = prepared.patch_sites.builtins().strong_relocations();
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
    decode_canonical::<scoop_slib::DecodedCanonicalDefinedLinkSymbolOwnerSetV1>(&encoded)
        .unwrap()
        .validate_against(&owners)
        .unwrap();
    let first = *actual_members.first().unwrap();
    let owner_bytes = encode(&scoop_slib::LinkDefinitionOwnerV1::OdrDefinition(first)).unwrap();
    let mut expected_owner_bytes = vec![0xa2, 0x00, 0x05, 0x01];
    expected_owner_bytes.extend(encode(&first).unwrap());
    assert_eq!(owner_bytes, expected_owner_bytes);
    let offset = encoded
        .windows(owner_bytes.len())
        .position(|window| window == owner_bytes)
        .unwrap();
    let mut changed = encoded.clone();
    changed[offset + owner_bytes.len() - 1] ^= 1;
    assert!(matches!(
        decode_canonical::<scoop_slib::DecodedCanonicalDefinedLinkSymbolOwnerSetV1>(&changed)
            .unwrap()
            .validate_against(&owners),
        Err(scoop_slib::DefinedLinkSymbolOwnerValidationError::ProjectionMismatch)
    ));
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
    let ordinary = scoop_slib::verify_cross_cone_strong_requirements_v1(
        prepared.target_selection.target(),
        closure.clone(),
        dependencies,
        public,
    )
    .unwrap();
    let shape =
        scoop_slib::verify_external_shape_requirements_v1(&ordinary, closure.producer(), imports)
            .unwrap();
    let undefined =
        crate::object_production::layout::complete_requirements(&prepared, &native, &shape)
            .unwrap();
    let finalized = prepared.finalize_metadata(&undefined).unwrap();
    if expected_bodies == 1 {
        member_fingerprints::check_inputs(
            &finalized,
            &prepared.foundation,
            prepared.production.canonical_callable_definitions(),
        );
    }
    finalization::check(
        &finalized,
        prepared.production.canonical_callable_definitions(),
    )
}

pub(super) fn check(
    emitted: &scoop_codegen::EmittedConeObjectSetV2,
) -> BTreeMap<PersistentCallableBodyId, Vec<u8>> {
    let production = emitted.production();
    let normalization = emitted.target().contract().native_symbol_normalization();
    let mut bodies = BTreeMap::new();
    for member in emitted.members() {
        let bytes = std::fs::read(member.path()).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        for definition in member.units().definition_plans() {
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
            assert_eq!(primary.is_weak(), weak);
            for boundary in plan.atom_boundaries() {
                let start = object
                    .symbol_by_name(&symbol_name(boundary.start()))
                    .unwrap();
                let end = object.symbol_by_name(&symbol_name(boundary.end())).unwrap();
                assert!(start.is_definition() && end.is_definition());
                assert_eq!(start.is_weak(), weak, "{}", symbol_name(boundary.start()));
                assert_eq!(end.is_weak(), weak, "{}", symbol_name(boundary.end()));
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
                    check_registration(data, body.as_array(), registration.definition_owner());
                    assert_eq!(&data[152..184], &[0; 32]);
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
                    check_registration(data, site.as_array(), registration.definition_owner());
                    assert_eq!(&data[200..232], &[0; 32]);
                }
                _ => {}
            }
        }
    }
    bodies
}

fn check_registration(data: &[u8], semantic: &[u8; 32], owner: RegistrationDefinitionOwner) {
    assert_eq!(&data[20..24], &[0; 4]);
    assert_eq!(&data[24..56], semantic);
    assert_eq!(&data[120..152], &[0; 32]);
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
