use super::*;
use scoop_identity::PersistentSafepointSiteId;
use scoop_slib::{
    CallableDefinitionAbiV1, ObjectDefinitionFingerprintV1, OdrMemberAbiV1, RegistrationAbiV1,
    VerifiedEntryPatchSetV2, VerifiedMaterializedPatchSiteV1,
};

#[derive(Debug, Eq, PartialEq)]
pub(in super::super) struct CallableFingerprints {
    pub(in super::super) object: ObjectDefinitionFingerprintV1,
    pub(in super::super) definition: CallableDefinitionAbiV1,
    pub(in super::super) registration: RegistrationAbiV1,
    pub(in super::super) safepoints: Vec<(
        PersistentSafepointSiteId,
        RegistrationAbiV1,
        scoop_slib::StackmapRecordFingerprintV1,
    )>,
}

impl CallableFingerprints {
    pub(in super::super) fn append_contents(&self, records: &mut Vec<Vec<u8>>) {
        records.push(self.object.as_array().to_vec());
        let CallableDefinitionAbiV1::Odr(definition) = self.definition else {
            panic!("the shared generic body must retain ODR ownership");
        };
        append_member(definition, records);
        append_registration(self.registration, records);
        for (site, fingerprint, stackmap) in &self.safepoints {
            records.push(stackmap.as_array().to_vec());
            records.push(site.as_array().to_vec());
            append_registration(*fingerprint, records);
        }
    }
}

fn append_registration(fingerprint: RegistrationAbiV1, records: &mut Vec<Vec<u8>>) {
    let RegistrationAbiV1::Odr(fingerprint) = fingerprint else {
        panic!("the shared generic body must retain ODR registrations");
    };
    append_member(fingerprint, records);
}

fn append_member(fingerprint: OdrMemberAbiV1, records: &mut Vec<Vec<u8>>) {
    records.push(fingerprint.abi().as_array().to_vec());
}

pub(super) fn check(
    finalized: &VerifiedEntryPatchSetV2,
    canonical: &scoop_lir::CanonicalCallableAbisV1,
) -> BTreeMap<PersistentCallableBodyId, CallableFingerprints> {
    let image = finalized.runtime_images().fingerprint();
    let registrations = image.registrations();
    let callables = registrations.callables();
    let plans = callables.body_objects().registrations();
    let mut fingerprints = BTreeMap::new();
    for ((computed, verified), plan) in callables
        .fingerprints()
        .iter()
        .zip(plans.registrations())
        .zip(plans.plan().registrations())
    {
        let fingerprint = computed.registration();
        check_identity(plan.definition_owner(), fingerprint);
        match (
            computed.definition(),
            canonical.get(computed.body()).unwrap().owner(),
        ) {
            (CallableDefinitionAbiV1::Strong, scoop_lir::CanonicalCallableAbiOwnerV1::Strong) => {}
            (
                CallableDefinitionAbiV1::Odr(value),
                scoop_lir::CanonicalCallableAbiOwnerV1::Odr {
                    group,
                    member,
                    role,
                    abi,
                },
            ) => {
                assert_eq!(
                    (value.group(), value.member(), value.role()),
                    (group, member, role)
                );
                assert_eq!(value.abi().as_array(), abi.as_array());
            }
            _ => panic!("the body fingerprint must use its actual owner"),
        }
        check_patch(
            finalized,
            verified.body_definition_patch(),
            computed.body_definition().as_array(),
        );
        assert!(
            fingerprints
                .insert(
                    computed.body(),
                    CallableFingerprints {
                        object: computed.body_definition(),
                        definition: computed.definition(),
                        registration: fingerprint,
                        safepoints: Vec::new(),
                    },
                )
                .is_none()
        );
    }
    let safepoints = registrations.safepoints();
    for ((computed, verified), plan) in safepoints
        .fingerprints()
        .iter()
        .zip(safepoints.registrations().registrations())
        .zip(safepoints.registrations().plan().registrations())
    {
        let fingerprint = computed.registration();
        check_identity(plan.definition_owner(), fingerprint);
        check_patch(
            finalized,
            verified.normalized_stackmap_patch(),
            computed.stackmap().as_array(),
        );
        fingerprints
            .get_mut(&plan.owner())
            .unwrap()
            .safepoints
            .push((plan.site(), fingerprint, computed.stackmap()));
    }
    assert_ne!(image.fingerprint().as_array(), &[0; 32]);
    check_patch(
        finalized,
        image.image().image_patch(),
        image.fingerprint().as_array(),
    );
    fingerprints
}

fn check_identity(owner: RegistrationDefinitionOwner, fingerprint: RegistrationAbiV1) {
    match (owner, fingerprint) {
        (RegistrationDefinitionOwner::Strong, RegistrationAbiV1::Strong) => {}
        (RegistrationDefinitionOwner::Odr { group, member }, RegistrationAbiV1::Odr(value)) => {
            assert_eq!(value.group(), group);
            assert_eq!(value.member(), member);
            assert_eq!(
                value.role(),
                scoop_identity::OdrMemberRole::RegistrationRecord
            );
            assert_ne!(value.abi().as_array(), &[0; 32]);
        }
        _ => panic!("the definition fingerprint must use the registration's actual owner"),
    }
}

fn check_patch(
    finalized: &VerifiedEntryPatchSetV2,
    patch: VerifiedMaterializedPatchSiteV1,
    expected: &[u8; 32],
) {
    let object = finalized
        .objects()
        .iter()
        .find(|object| object.member() == patch.member())
        .unwrap();
    let start = usize::try_from(patch.checked_offset()).unwrap();
    assert_eq!(patch.width_bytes(), 32);
    assert_eq!(&object.bytes()[start..start + 32], expected);
}
