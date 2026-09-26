use super::*;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    expected: &[mir::MirTypeBridgeDependencyV1],
) {
    let local = &input.hir.output().local;
    let string = input
        .mir
        .module()
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .unwrap();
    assert_eq!(
        string.identity_record(),
        &local.exact_type_identities[local.string]
    );
    // Runtime descriptor metadata does not invent a source value-layout use.
    assert!(expected.iter().all(|usage| {
        usage.target() != mir::MirTypeBridgeTargetV1::Type(string.identity_record().id())
    }));
    assert_eq!(
        scoop_mir_lower::lower_type_bridge_dependencies(input).unwrap(),
        expected
    );
}
