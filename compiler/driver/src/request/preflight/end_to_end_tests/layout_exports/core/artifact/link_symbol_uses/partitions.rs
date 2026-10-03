use super::*;
use std::collections::BTreeSet;

pub(super) fn check(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) {
    let strong = proof
        .object_contents()
        .patch_sites()
        .builtins()
        .strong_relocations();
    assert_eq!(
        proof.defined_symbols(),
        &slib::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(strong).unwrap()
    );
    let expected = strong
        .bindings()
        .iter()
        .filter(|binding| {
            !matches!(
                binding.resolution(),
                slib::StrongRelocationResolutionV1::ObjectLocalStrong { .. }
            )
        })
        .map(slib::CanonicalUndefinedRelocationUseV1::from)
        .collect::<Vec<_>>();
    let partitions = proof.undefined_partitions();
    let legacy = partitions.legacy().requirements();
    let ordinary = partitions.cross_cone().requirements();
    let shape = partitions.external_shape();
    let mut actual = legacy
        .iter()
        .map(|use_| use_.use_site().clone())
        .chain(ordinary.iter().map(|use_| use_.use_site().clone()))
        .chain(shape.iter().map(|use_| use_.use_site().clone()))
        .collect::<Vec<_>>();
    let key = |use_: &slib::CanonicalUndefinedRelocationUseV1| {
        (
            use_.source_member(),
            use_.containing_atom(),
            use_.offset_within_atom(),
            use_.target_slot(),
        )
    };
    actual.sort_unstable_by_key(key);
    assert_eq!(
        actual.iter().map(key).collect::<BTreeSet<_>>().len(),
        actual.len()
    );
    assert_eq!(actual, expected);
}
