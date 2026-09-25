use super::*;
use hir::{DefaultSourceReferenceClosureError as Error, ExportDefaultReferenceKindV1 as Kind};
use scoop_wire::WirePath;

#[test]
fn each_source_reference_domain_rejects_missing_extra_reordered_origin_and_target_corruption() {
    with_template(|template| {
        for kind in [
            Kind::Callable,
            Kind::Constructor,
            Kind::Type,
            Kind::Global,
            Kind::Singleton,
            Kind::Field,
        ] {
            for change in [
                Change::Missing,
                Change::Extra,
                Change::Reordered,
                Change::Origin,
                Change::Target,
            ] {
                let changed = changed(template, kind, change);
                let error = changed
                    .bind_reference_occurrences(&WirePath::root())
                    .unwrap_err();
                let actual = match (&change, &error) {
                    (Change::Missing, Error::Missing { kind, .. })
                    | (Change::Extra, Error::Extra { kind, .. })
                    | (
                        Change::Reordered,
                        Error::Origin { kind, .. } | Error::Target { kind, .. },
                    )
                    | (Change::Origin, Error::Origin { kind, .. })
                    | (Change::Target, Error::Target { kind, .. }) => *kind,
                    _ => panic!("{kind:?}, {change:?}: {error:?}"),
                };
                assert_eq!(actual, kind);
            }
        }
    });
}

#[test]
fn repeated_occurrences_keep_distinct_sequence_positions_and_borrow_the_source_witness() {
    with_template(|template| {
        let bound = template
            .bind_reference_occurrences(&WirePath::root())
            .unwrap();
        let mut next = std::collections::BTreeMap::new();
        for occurrence in bound.occurrences() {
            let kind = occurrence.source().kind();
            let index = next.entry(kind).or_insert(0);
            assert_eq!(occurrence.index(), *index);
            *index += 1;
            if let hir::DefaultSourceReferenceRecordV1::Callable(record) = occurrence.source() {
                assert!(std::ptr::eq(
                    record,
                    &template.references().callables()[occurrence.index() as usize]
                ));
                assert!(std::ptr::eq(
                    occurrence.source().witness(),
                    record.witness()
                ));
            }
        }
        assert_eq!(next.len(), 6);
        assert_eq!(
            template.references().callables()[0].target(),
            template.references().callables()[2].target()
        );
    });
}
