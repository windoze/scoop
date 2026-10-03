use super::*;
use hir::concrete::TypeKind;
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-fact-providers/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-fact-providers/combined.scoop"
));

#[test]
fn fact_providers_keep_source_nominals_and_structural_support_in_their_actual_cones() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            let local = output.output().local.module();
            let records = production.exact_facts().records();
            assert_eq!(records.len(), if case == "combined" { 27 } else { 5 });
            for fact in production.exact_facts().records() {
                let ty = local
                    .exact_type_identities
                    .type_for_identity(fact.exact())
                    .unwrap();
                assert!(!matches!(
                    local.types[ty].kind,
                    TypeKind::Unit
                        | TypeKind::Any
                        | TypeKind::Integer(_)
                        | TypeKind::Boolean
                        | TypeKind::String
                ));
                if matches!(
                    local.types[ty].kind,
                    TypeKind::Class(_) | TypeKind::Interface(_)
                ) {
                    assert_eq!(fact.kind(), hir::ExactTypeKindV1::Reference);
                    assert_eq!(fact.gc(), hir::ExactTypeGcV1::ContainsManagedReferences);
                } else {
                    assert!(matches!(fact.kind(), hir::ExactTypeKindV1::Value { .. }));
                }
            }
            let gc_free = records.iter().filter(|fact| fact.gc().is_gc_free()).count();
            let zero_sized = records
                .iter()
                .filter(|fact| {
                    matches!(
                        fact.kind(),
                        hir::ExactTypeKindV1::Value {
                            zst: hir::ZstStatus::ZeroSized
                        }
                    )
                })
                .count();
            assert_eq!(
                (gc_free, zero_sized),
                if case == "combined" { (5, 3) } else { (1, 0) }
            );
        });
    }
}
