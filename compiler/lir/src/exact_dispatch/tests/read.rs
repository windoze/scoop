use super::*;
use crate::{CanonicalExactLayoutExportsV1, ConeLirFoundation};
use scoop_identity::*;

pub(super) fn identities(
    foundation: &ConeLirFoundation,
    records: &[ExactDispatchExportV1],
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(foundation.producer()).unwrap();
    let mut exacts = std::collections::BTreeSet::new();
    let mut slots = std::collections::BTreeSet::new();
    let mut bodies = std::collections::BTreeSet::new();
    let mut functions = std::collections::BTreeSet::new();
    for record in records {
        pending.register_authority(record.table()).unwrap();
        pending
            .register_authority(record.definition().definition_plan())
            .unwrap();
        exacts.insert(record.owner_exact());
        if let ExactDispatchRoleV1::Itable { interface_exact } = record.role() {
            exacts.insert(interface_exact);
        }
        for entry in record.entries() {
            slots.insert(entry.slot());
            let signature = entry.slot_signature().exact();
            exacts.extend(signature.receiver().into_option());
            exacts.extend(signature.parameters().iter().copied());
            exacts.insert(signature.result());
            let CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::Function(function)) =
                entry.implementation().target()
            else {
                panic!("dispatch fixtures use source functions");
            };
            functions.insert(function);
            if let StrongTypeDispatchCallableRefV2::Local(body) = entry.abi() {
                bodies.insert(body);
            }
        }
    }
    for id in exacts {
        pending.register_authority(id).unwrap();
    }
    for id in bodies {
        pending.register_authority(id).unwrap();
    }
    for id in slots {
        pending.register_authority(id).unwrap();
    }
    for id in functions {
        pending.register_authority(id).unwrap();
    }
    pending.finish().unwrap()
}

pub(super) fn roundtrip(fixture: &DirectFixture, record: &ExactDispatchExportV1) {
    let bytes = encode(record).unwrap();
    let decoded = decode_canonical::<DecodedExactDispatchExportV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let bound =
        crate::exact_layout::tests::Bound::value(fixture.owner.identity().exact_record().clone());
    let layouts = CanonicalExactLayoutExportsV1::try_new(
        TARGET,
        &bound.foundation,
        vec![fixture.owner.clone()],
    )
    .unwrap();
    let mut graph = identities(&fixture.foundation, std::slice::from_ref(record));
    assert_eq!(
        decoded
            .read(TARGET, &fixture.foundation, &[&layouts], &mut graph)
            .unwrap(),
        *record
    );
}
