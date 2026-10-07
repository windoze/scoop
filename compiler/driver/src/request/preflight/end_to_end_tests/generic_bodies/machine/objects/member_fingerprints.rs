use scoop_lir::{
    CanonicalCallableAbiOwnerV1, CanonicalCallableAbiV1, CanonicalCallableAbisV1, ConeLirFoundation,
};
use scoop_slib::{CallableDefinitionAbiV1, VerifiedEntryPatchSetV2};
use scoop_wire::Digest256;

pub(super) fn check_inputs(
    finalized: &VerifiedEntryPatchSetV2,
    foundation: &ConeLirFoundation,
    canonical: &CanonicalCallableAbisV1,
) {
    let callables = finalized
        .runtime_images()
        .fingerprint()
        .registrations()
        .callables();
    let original = callables
        .fingerprints()
        .iter()
        .find(|body| matches!(body.definition(), CallableDefinitionAbiV1::Odr(_)))
        .unwrap();
    let CallableDefinitionAbiV1::Odr(original_member) = original.definition() else {
        unreachable!("selected ODR body")
    };
    let leaf = *canonical.get(original.body()).unwrap();
    let CanonicalCallableAbiOwnerV1::Odr {
        group,
        member,
        role,
        abi,
    } = leaf.owner()
    else {
        panic!("ODR canonical body")
    };
    let mut digest = *abi.as_array();
    digest[0] ^= 1;
    let replacement = CanonicalCallableAbiV1::new(
        leaf.body(),
        CanonicalCallableAbiOwnerV1::Odr {
            group,
            member,
            role,
            abi: Digest256::from_array(digest),
        },
    );
    let definitions = canonical
        .definitions()
        .iter()
        .map(|record| {
            if record.body() == leaf.body() {
                replacement
            } else {
                *record
            }
        })
        .collect();
    let changed = CanonicalCallableAbisV1::new(definitions, foundation).unwrap();
    let recomputed = scoop_slib::compute_strong_callable_fingerprints_v1(
        callables.body_objects().clone(),
        &changed,
    )
    .unwrap();
    for (before, after) in callables
        .fingerprints()
        .iter()
        .zip(recomputed.fingerprints())
    {
        if before.body() != leaf.body() {
            assert_eq!(before, after);
            continue;
        }
        assert_eq!(before.body_definition(), after.body_definition());
        assert_eq!(before.registration(), after.registration());
        let CallableDefinitionAbiV1::Odr(after) = after.definition() else {
            panic!("ODR definition retained");
        };
        assert_ne!(original_member.abi(), after.abi());
    }
}
