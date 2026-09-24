use super::*;
use hir::{
    CanonicalProtectedDefaultSlotCallDomainsV1, PersistentAccessDomainV1, PersistentLookupDomainV1,
    PersistentSlotContractDomainV1, ProtectedDefaultAccessWitnessV1,
    ProtectedDefaultBodyClosureError, ProtectedDefaultCallableReferenceV1,
    ProtectedDefaultSlotCallDomainV1,
};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let mut tested_slots = false;
    for template in checked.section().protected_defaults().records() {
        let Some(reference) = template.references().callables().first() else {
            continue;
        };
        let mut altered = Vec::new();
        match reference.witness().view() {
            ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { .. } => {
                altered.push(
                    ProtectedDefaultAccessWitnessV1::param_free(
                        template.key().owner(),
                        PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
                        CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
                        PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal()),
                    )
                    .unwrap(),
                );
            }
            ProtectedDefaultAccessWitnessViewV1::ParamFree(witness) => {
                altered.push(
                    ProtectedDefaultAccessWitnessV1::generic_source_metadata(
                        template.key().owner(),
                    )
                    .unwrap(),
                );
                for (direct, target) in [
                    (
                        different(witness.direct_call_domain().domain()),
                        witness.target_domain().domain().clone(),
                    ),
                    (
                        witness.direct_call_domain().domain().clone(),
                        different(witness.target_domain().domain()),
                    ),
                ] {
                    altered.push(
                        ProtectedDefaultAccessWitnessV1::param_free(
                            template.key().owner(),
                            PersistentLookupDomainV1::new(direct),
                            witness.slot_call_domains().clone(),
                            PersistentLookupDomainV1::new(target),
                        )
                        .unwrap(),
                    );
                }
                if let Some(slot) = witness.slot_call_domains().records().first() {
                    tested_slots = true;
                    let mut changed = witness.slot_call_domains().records().to_vec();
                    changed[0] = ProtectedDefaultSlotCallDomainV1::new(
                        slot.slot(),
                        PersistentSlotContractDomainV1::new(different(slot.domain().domain())),
                    );
                    for slots in [Vec::new(), changed] {
                        altered.push(
                            ProtectedDefaultAccessWitnessV1::param_free(
                                template.key().owner(),
                                witness.direct_call_domain().clone(),
                                CanonicalProtectedDefaultSlotCallDomainsV1::try_new(slots).unwrap(),
                                witness.target_domain().clone(),
                            )
                            .unwrap(),
                        );
                    }
                }
            }
        }
        for witness in altered {
            let changed = ProtectedDefaultCallableReferenceV1::new(
                reference.target().clone(),
                reference.definition_origin().clone(),
                witness,
                reference.uses().clone(),
            );
            assert!(matches!(
                references::reject_reference(checked, core, template, changed),
                Error::DefaultBody(error)
                    if matches!(error.as_ref(),
                        ProtectedDefaultBodyClosureError::Source(Error::DefaultWitness(key))
                            if *key == template.key())
            ));
        }
    }
    assert!(tested_slots);
}

fn different(domain: &PersistentAccessDomainV1) -> PersistentAccessDomainV1 {
    if domain.is_universal() {
        PersistentAccessDomainV1::empty()
    } else {
        PersistentAccessDomainV1::universal()
    }
}
