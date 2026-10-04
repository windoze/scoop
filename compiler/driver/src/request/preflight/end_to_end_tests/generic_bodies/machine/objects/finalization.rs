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
        let [(site, RegistrationFingerprintV1::Odr(safepoint))] = value.safepoints.as_slice()
        else {
            panic!("the standalone fixture has exactly one ODR safepoint");
        };
        assert_eq!(
            site.to_string(),
            "6700f64aa96c8984f20476cbd989168e67404473137c1f633804b0d837c53da2"
        );
        assert_eq!(
            [
                [
                    body.abi().to_string(),
                    body.lir().to_string(),
                    body.definition().to_string()
                ],
                [
                    fingerprint.abi().to_string(),
                    fingerprint.lir().to_string(),
                    fingerprint.definition().to_string()
                ],
                [
                    safepoint.abi().to_string(),
                    safepoint.lir().to_string(),
                    safepoint.definition().to_string()
                ],
            ],
            [
                [
                    "7ec9e0f465e302f9a859d00069abeef4b3712d22336c6586eb018b9802186886",
                    "d813d5daaa1eeb9571728b9634c3f537a5b551cb8e3a49478a9262f01db92310",
                    "03437cc06b26302727fc48d13bb93a72e922609f80667dcf86819815dc413bce",
                ],
                [
                    "1850372d03688c173adc5273d1e6e4f3b7c4bbdee16c8648f4d150e05815f9d0",
                    "586f0cb7b9a3dccddaff36bc10d6325e4d52f3743670ec878a216e5991d3d7c5",
                    "fc793cf6db11667cda7dcbaa85d377b84723fb23ff5d12308e384d8ef7ad2445",
                ],
                [
                    "d2091017b8eb5710b4bb2bfcadfbe436fb0f431b37d409204e03270bd23c808e",
                    "6f1d8f27e7b503ff9f9fe6a2086b4f61f5aa473bbf3d0df0a471eb06897d1443",
                    "91cd129d3d91846992c9afec329693f972f8791a98a3a9776a828b0aab7f64d9",
                ],
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
