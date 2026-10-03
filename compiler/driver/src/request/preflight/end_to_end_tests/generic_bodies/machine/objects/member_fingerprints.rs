use scoop_lir::{
    CanonicalCallableDefinitionOwnerV1, CanonicalCallableLirDefinitionV1,
    CanonicalCallableLirDefinitionsV1, ConeLirFoundation,
};
use scoop_slib::{CallableDefinitionFingerprintV1, VerifiedEntryPatchSetV2};
use scoop_wire::Digest256;

pub(super) fn check_inputs(
    finalized: &VerifiedEntryPatchSetV2,
    foundation: &ConeLirFoundation,
    canonical: &CanonicalCallableLirDefinitionsV1,
) {
    let callables = finalized
        .runtime_images()
        .fingerprint()
        .registrations()
        .callables();
    let original = callables
        .fingerprints()
        .iter()
        .find(|body| matches!(body.definition(), CallableDefinitionFingerprintV1::Odr(_)))
        .unwrap();
    let CallableDefinitionFingerprintV1::Odr(original_member) = original.definition() else {
        unreachable!("selected ODR body")
    };
    let leaf = *canonical.get(original.body()).unwrap();
    let CanonicalCallableDefinitionOwnerV1::Odr {
        group,
        member,
        role,
        abi,
    } = leaf.owner()
    else {
        panic!("ODR canonical body")
    };
    for change_lir in [true, false] {
        let mut digest = *if change_lir { leaf.fingerprint() } else { abi }.as_array();
        digest[0] ^= 1;
        let changed = Digest256::from_array(digest);
        let replacement = if change_lir {
            CanonicalCallableLirDefinitionV1::new(leaf.body(), changed, leaf.owner())
        } else {
            CanonicalCallableLirDefinitionV1::new(
                leaf.body(),
                leaf.fingerprint(),
                CanonicalCallableDefinitionOwnerV1::Odr {
                    group,
                    member,
                    role,
                    abi: changed,
                },
            )
        };
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
        let changed = CanonicalCallableLirDefinitionsV1::new(definitions, foundation).unwrap();
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
            assert_eq!(before.registration_object(), after.registration_object());
            assert_eq!(before.registration(), after.registration());
            let CallableDefinitionFingerprintV1::Odr(after) = after.definition() else {
                panic!("ODR definition retained");
            };
            if change_lir {
                assert_eq!(original_member.abi(), after.abi());
                assert_ne!(original_member.lir(), after.lir());
                assert_ne!(original_member.definition(), after.definition());
            } else {
                assert_ne!(original_member.abi(), after.abi());
                assert_eq!(original_member.lir(), after.lir());
                assert_eq!(original_member.definition(), after.definition());
            }
        }
    }
}
