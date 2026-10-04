//! The real core keeps its concrete nominal applications across stages.

use std::collections::BTreeSet;

pub(super) fn check(
    hir: &scoop_hir::DependencyHirOutput,
    foundation: &scoop_hir::CanonicalHirFoundation,
    mir: &scoop_mir::Module,
    lir: &scoop_lir::Module,
) {
    let local = hir.output().local.module();
    let mut groups: BTreeSet<_> = hir
        .output()
        .local
        .module()
        .exact_type_identities
        .nominal_specialization_records()
        .iter()
        .map(|record| record.id())
        .collect();
    let coroutine_types = local
        .coroutine_protocols
        .iter()
        .flat_map(|protocol| {
            [
                protocol.continuation,
                protocol.suspend_task,
                protocol.suspend_registration,
            ]
            .map(|interface| {
                local.exact_type_identities[local.interfaces[interface].canonical_type].id()
            })
        })
        .collect::<BTreeSet<_>>();
    let mut coroutine_descriptors = BTreeSet::new();
    let mut applications = BTreeSet::new();
    for (_, descriptor) in lir.meta.type_descriptors.iter() {
        let Some(member) = descriptor.identity.odr_member_record() else {
            continue;
        };
        let exact = descriptor.identity.exact_type();
        let Some(source) = mir.meta.source_exact_types.get_by_identity(exact) else {
            continue;
        };
        let scoop_mir::SourceExactTypeOwner::NominalApplication(group) = source.owner() else {
            continue;
        };
        if coroutine_types.contains(&exact) {
            assert!(coroutine_descriptors.insert(exact));
        } else {
            assert!(applications.insert(scoop_mir::type_name(mir, source.ty())));
        }
        assert_eq!(group, member.key().group());
        assert!(groups.contains(&member.key().group()));
        assert_eq!(
            descriptor.identity.symbol_request().linkage(),
            scoop_identity::LinkageClass::OdrWeak
        );
        assert!(
            lir.meta
                .layouts
                .iter()
                .any(|(_, layout)| { layout.identity.layout_record().key().exact_type() == exact }),
            "each materialized descriptor keeps its matching exact layout"
        );
    }
    let expected = [
        "Array<Char>",
        "Array<Int8>",
        "ArrayList<String>",
        "Iterable<Char>",
        "Iterable<Int8>",
        "Iterable<Int>",
        "Iterable<Long>",
        "Iterable<Option<String>>",
        "Iterable<String>",
        "Iterable<UInt>",
        "Iterable<ULong>",
        "Iterator<Char>",
        "Iterator<Int8>",
        "Iterator<Int>",
        "Iterator<Long>",
        "Iterator<Option<String>>",
        "Iterator<String>",
        "Iterator<UInt>",
        "Iterator<ULong>",
        "List<Char>",
        "List<Int8>",
        "List<Option<String>>",
        "List<String>",
        "ListIterator<Char>",
        "ListIterator<Int8>",
        "ListIterator<Option<String>>",
        "ListIterator<String>",
        "MutableArray<Char>",
        "MutableArray<Int8>",
        "MutableArray<Option<String>>",
        "MutableList<String>",
        "Option<(Long, Long)>",
        "Option<Char>",
        "Option<Int8>",
        "Option<Int>",
        "Option<Long>",
        "Option<Option<String>>",
        "Option<String>",
        "Option<UInt>",
        "Option<ULong>",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(applications, expected);
    assert_eq!(coroutine_descriptors, coroutine_types);

    let callables = &local.callable_applications;
    groups.extend(
        callables
            .odr_group_records()
            .iter()
            .map(|record| record.id()),
    );
    let counts = foundation.counts();
    assert_eq!(counts.callable_applications, callables.len());
    assert_eq!(counts.odr_groups, groups.len());
    assert_eq!(counts.odr_members, callables.odr_member_records().len());
}
