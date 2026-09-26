use super::*;
use crate::{ExternalStrongShapeSubjectV1 as Subject, StrongShapeDefinitionRefV1};

pub(super) fn foreign_fixture() -> Fixture {
    Fixture::new(Options {
        first_type_has_itable: true,
        first_itable_interface: Some(foreign_exact("Interface")),
        ..Options::default()
    })
}

pub(super) fn catalog(
    semantics: &StrongTypeDescriptorSemanticPlanSetV2,
) -> crate::StrongTypeReferenceDefinitionsV2 {
    let mut subjects = std::collections::BTreeSet::new();
    for descriptor in semantics.descriptors() {
        if let Some(Descriptor::DependencyExternal { exact, .. }) = descriptor.parent() {
            subjects.insert(Subject::TypeDescriptor(exact));
        }
        for table in descriptor.itables() {
            if let Descriptor::DependencyExternal { exact, .. } = table.interface() {
                subjects.insert(Subject::TypeDescriptor(exact));
            }
        }
        for slot in descriptor
            .vtable()
            .slots()
            .iter()
            .chain(descriptor.itables().iter().flat_map(|table| table.slots()))
        {
            if let Callable::DependencyExternal { body, .. } = slot {
                assert_eq!(*body, foreign_body());
                subjects.insert(Subject::Callable(foreign_owner()));
            }
        }
    }
    let definitions = subjects
        .into_iter()
        .map(physical_definition)
        .collect::<Vec<_>>();
    crate::StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &definitions).unwrap()
}

fn physical_definition(subject: Subject) -> StrongShapeDefinitionRefV1 {
    let (key, symbol) = subject.expected_definition(ConeIdentity::CORE).unwrap();
    let definition = CborIdentityRecord::from_key(key).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![
            PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap(),
        ])
        .unwrap(),
    );
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    StrongShapeDefinitionRefV1::from_foundation(subject, &foundation).unwrap()
}

pub(super) fn decoded(
    plans: &StrongTypeRegistrationPlanSetV2,
) -> Vec<DecodedStrongTypeRegistrationPlanV2> {
    plans
        .registrations()
        .iter()
        .map(|plan| decode_canonical(&encode(plan).unwrap()).unwrap())
        .collect()
}

pub(super) fn validate(
    fixture: &Fixture,
    records: Vec<DecodedStrongTypeRegistrationPlanV2>,
    definitions: &crate::StrongTypeReferenceDefinitionsV2,
) -> Result<StrongTypeDescriptorSemanticPlanSetV2, crate::StrongRegistrationProductionValidationError>
{
    crate::validate_type_registration_constituents_v2(
        records,
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        definitions,
        &fixture.digests,
    )
}
