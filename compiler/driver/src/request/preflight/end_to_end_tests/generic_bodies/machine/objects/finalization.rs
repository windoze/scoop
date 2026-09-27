use super::*;
use scoop_identity::{DigestKind, PersistentSafepointSiteId};
use scoop_slib::{
    CallableDefinitionFingerprintV1, ObjectDefinitionFingerprintV1, OdrMemberFingerprintV1,
    RegistrationFingerprintV1, VerifiedEntryPatchSetV2, VerifiedMaterializedPatchSiteV1,
};

#[derive(Debug, Eq, PartialEq)]
pub(in super::super) struct CallableFingerprints {
    pub(in super::super) objects: [ObjectDefinitionFingerprintV1; 2],
    pub(in super::super) definition: CallableDefinitionFingerprintV1,
    pub(in super::super) registration: RegistrationFingerprintV1,
    pub(in super::super) safepoints: Vec<(PersistentSafepointSiteId, RegistrationFingerprintV1)>,
}

impl CallableFingerprints {
    pub(in super::super) fn append_contents(&self, records: &mut Vec<Vec<u8>>) {
        records.extend(self.objects.iter().map(|value| value.as_array().to_vec()));
        let CallableDefinitionFingerprintV1::Odr(definition) = self.definition else {
            panic!("the shared generic body must retain ODR ownership");
        };
        append_member(definition, records);
        append_registration(self.registration, records);
        for (site, fingerprint) in &self.safepoints {
            records.push(site.as_array().to_vec());
            append_registration(*fingerprint, records);
        }
    }
}

fn append_registration(fingerprint: RegistrationFingerprintV1, records: &mut Vec<Vec<u8>>) {
    let RegistrationFingerprintV1::Odr(fingerprint) = fingerprint else {
        panic!("the shared generic body must retain ODR registrations");
    };
    append_member(fingerprint, records);
}

fn append_member(fingerprint: OdrMemberFingerprintV1, records: &mut Vec<Vec<u8>>) {
    records.push(fingerprint.abi().as_array().to_vec());
    records.push(fingerprint.lir().as_array().to_vec());
    records.push(fingerprint.definition().as_array().to_vec());
}

pub(super) fn check(
    finalized: &VerifiedEntryPatchSetV2,
    canonical: &scoop_lir::CanonicalCallableLirDefinitionsV1,
) -> BTreeMap<PersistentCallableBodyId, CallableFingerprints> {
    let image = finalized.runtime_images().fingerprint();
    let registrations = image.registrations();
    assert!(matches!(
        scoop_slib::CanonicalStrongRegistrationFingerprintSetV1::from_patch_set(registrations),
        Err(scoop_slib::StrongRegistrationFingerprintProjectionError::OdrRegistration { .. })
    ));
    let callables = registrations.callables();
    let plans = callables
        .body_objects()
        .registration_objects()
        .registrations();
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
            (
                CallableDefinitionFingerprintV1::Strong(value),
                scoop_lir::CanonicalCallableDefinitionOwnerV1::Strong,
            ) => assert_eq!(value, computed.body_definition()),
            (
                CallableDefinitionFingerprintV1::Odr(value),
                scoop_lir::CanonicalCallableDefinitionOwnerV1::Odr {
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
                assert_eq!(
                    value.lir(),
                    canonical.get(computed.body()).unwrap().fingerprint()
                );
                assert_ne!(value.definition().as_array(), &[0; 32]);
            }
            _ => panic!("the body fingerprint must use its actual owner"),
        }
        assert_eq!(
            verified.registration_definition_patch().source(),
            computed.registration_node()
        );
        check_patch(
            finalized,
            verified.registration_definition_patch(),
            fingerprint.as_array(),
        );
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
                        objects: [computed.body_definition(), computed.registration_object()],
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
        assert_eq!(
            verified.registration_definition_patch().source(),
            computed.registration_node()
        );
        check_patch(
            finalized,
            verified.registration_definition_patch(),
            fingerprint.as_array(),
        );
        check_patch(
            finalized,
            verified.normalized_stackmap_patch(),
            computed.stackmap().as_array(),
        );
        fingerprints
            .get_mut(&plan.owner())
            .unwrap()
            .safepoints
            .push((plan.site(), fingerprint));
    }
    assert_ne!(image.fingerprint().as_array(), &[0; 32]);
    check_patch(
        finalized,
        image.image().image_patch(),
        image.fingerprint().as_array(),
    );
    let odr = fingerprints
        .values()
        .filter_map(|value| match value.registration {
            RegistrationFingerprintV1::Odr(fingerprint) => Some((value, fingerprint)),
            RegistrationFingerprintV1::Strong(_) => None,
        })
        .collect::<Vec<_>>();
    if let [(value, fingerprint)] = odr.as_slice() {
        let CallableDefinitionFingerprintV1::Odr(body) = value.definition else {
            panic!("the standalone body has ODR ownership");
        };
        assert_eq!(
            [
                body.abi().to_string(),
                body.lir().to_string(),
                body.definition().to_string()
            ],
            [
                "7ec9e0f465e302f9a859d00069abeef4b3712d22336c6586eb018b9802186886",
                "4579303ef729082681e4005f1e11b2abb4770d084be715a8fdfc673dbca02cd5",
                "a6abc9a21a3a3991a69c4d79926d16dc6f5d4d4d33b3c2ce596d7a78d41d316e",
            ],
        );
        assert_eq!(
            [
                fingerprint.abi().to_string(),
                fingerprint.lir().to_string(),
                fingerprint.definition().to_string()
            ],
            [
                "444577cffc1676a63c719618650715c2f3e4df2f2c76e44a05e132583639ef6f",
                "7292bd1b0847e93ba208769ae58a6299e867455a9a839306630b69e6e06ed823",
                "045ebdf387d6ab1268475273284e028f624323946597f894646fa85d07ebb293",
            ],
        );
        let [(site, RegistrationFingerprintV1::Odr(fingerprint))] = value.safepoints.as_slice()
        else {
            panic!("the standalone fixture has exactly one ODR safepoint");
        };
        assert_eq!(
            site.to_string(),
            "1aa90338a1e6a0bda6373b6a9fedfe94bea9f55cdd9e515d1cac30ec77e8869c"
        );
        assert_eq!(
            [
                fingerprint.abi().to_string(),
                fingerprint.lir().to_string(),
                fingerprint.definition().to_string()
            ],
            [
                "42493ae134fa6ec88963d121ace39b43a2008496815441327a8d874b27a632eb",
                "0f8764406e975b43f915ccf2fd867585134adf7cea4c98a167c91f0db37dc78b",
                "3121df8cb5b3d26e26c35b3698b11a8418f40311eb064a66127f314b19d3fa33",
            ],
        );
    }
    fingerprints
}

fn check_identity(owner: RegistrationDefinitionOwner, fingerprint: RegistrationFingerprintV1) {
    match (owner, fingerprint) {
        (RegistrationDefinitionOwner::Strong, RegistrationFingerprintV1::Strong(value)) => {
            assert_eq!(fingerprint.kind(), DigestKind::StrongRegistration);
            assert_ne!(value.as_array(), &[0; 32]);
        }
        (
            RegistrationDefinitionOwner::Odr { group, member },
            RegistrationFingerprintV1::Odr(value),
        ) => {
            assert_eq!(fingerprint.kind(), DigestKind::OdrDefinition);
            assert_eq!(value.group(), group);
            assert_eq!(value.member(), member);
            assert_eq!(
                value.role(),
                scoop_identity::OdrMemberRole::RegistrationRecord
            );
            assert_ne!(value.abi().as_array(), &[0; 32]);
            assert_ne!(value.lir().as_array(), &[0; 32]);
            assert_ne!(value.definition().as_array(), &[0; 32]);
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
