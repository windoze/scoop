use std::collections::BTreeMap;

use object::{Object, ObjectSection, ObjectSymbol};
use scoop_identity::{
    LinkageClass, PersistentCallableBodyId, StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_lir::RegistrationDefinitionOwner;

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
