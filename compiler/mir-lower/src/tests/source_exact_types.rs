use std::collections::HashSet;

use super::*;

#[test]
fn mir_records_exact_identities_without_materializing_unused_types() {
    let mut harness = Harness::new();
    let int = harness.int;
    let boolean = harness.boolean;
    let pair = harness.tuple(&[int, boolean]);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", pair));
    let entry = harness.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                value,
                expr(
                    hir::ExprKind::TupleLiteral(vec![
                        int_lit(&harness, 1),
                        expr(hir::ExprKind::BoolLiteral(true), boolean),
                    ]),
                    pair,
                ),
            )],
        },
    );
    let export = harness.finish(entry);
    let concrete =
        scoop_hir_lower::concretize_output(&export).expect("concrete type applications are valid");
    let module = lower(&export);

    let expected = concrete
        .module()
        .types
        .iter()
        .map(|(ty, _)| concrete.module().exact_type_identities[ty].id())
        .collect::<HashSet<_>>();
    let actual = module
        .meta
        .source_exact_types
        .iter()
        .map(|entry| entry.identity_record().id())
        .collect::<HashSet<_>>();
    assert!(actual.is_subset(&expected));
    assert!(actual.len() < expected.len());
    assert!(
        module
            .meta
            .source_exact_types
            .get(&mir::Type::Tuple(vec![
                mir::Type::Integer(mir::IntegerKind::SIGNED_32),
                mir::Type::Boolean,
            ]))
            .is_some()
    );
    module.validate().unwrap();
}
