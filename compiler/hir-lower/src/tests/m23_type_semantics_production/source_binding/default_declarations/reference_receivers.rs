use super::*;
use hir::{DefaultExpressionReferenceReceiverV1 as Receiver, DefaultReferenceContextV1 as Context};
use std::fmt::Write;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/reference-receivers.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/reference-receiver-combinations.scoop"
));

#[test]
fn source_reference_receivers_keep_this_explicit_super_and_constructor_contexts() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
            let mut snapshot = String::new();
            for (name, position) in [
                ("ReceiverHost.ownField", 0),
                ("ReceiverHost.implicit", 0),
                ("ReceiverHost.explicit", 1),
                ("ReceiverHost.field", 1),
                ("ReceiverHost.parent", 0),
                ("ReceiverHost.construct", 0),
            ] {
                let contract = bound.declaration(key(output, name, position)).unwrap();
                let mut references = 0;
                for occurrence in contract.references().occurrences() {
                    if matches!(
                        occurrence.source().kind(),
                        hir::ExportDefaultReferenceKindV1::Type
                    ) {
                        continue;
                    }
                    let Context::Expression { index, receiver } = occurrence.context() else {
                        panic!("expected expression reference");
                    };
                    references += 1;
                    write!(
                        snapshot,
                        "{name}: {:?} expression {}",
                        occurrence.source().kind(),
                        index
                    )
                    .unwrap();
                    match receiver {
                        Receiver::None => writeln!(snapshot).unwrap(),
                        Receiver::Member {
                            expression,
                            implicit_this,
                            direct_super,
                        } => {
                            let hir::DefaultExpressionKindV1::Local(local) = expression.kind()
                            else {
                                panic!("expected receiver local")
                            };
                            assert_eq!(implicit_this, local == &LocalValueSelector::This);
                            writeln!(snapshot, "; this {implicit_this}; super {direct_super}")
                                .unwrap();
                        }
                    }
                }
                assert_eq!(references, 1, "{name}");
            }
            assert_eq!(
                snapshot,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-source-defaults/reference-receivers.snap"
                ))
            );
        });
    });
}

#[test]
fn source_reference_receivers_preserve_nested_captures_and_assignment_metadata() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
            let mut captures = 0;
            let mut local_functions = 0;
            let mut assignments = 0;
            let mut explicit = 0;
            for contract in bound.declarations() {
                for occurrence in contract.references().occurrences() {
                    match occurrence.context() {
                        Context::Metadata(metadata) => {
                            assert!(occurrence.context().expression_index().is_none());
                            match metadata {
                                hir::DefaultBodyReferenceMetadataV1::Capture(_) => captures += 1,
                                hir::DefaultBodyReferenceMetadataV1::LocalFunction(_) => {
                                    local_functions += 1
                                }
                                hir::DefaultBodyReferenceMetadataV1::Assignment(target) => {
                                    assert!(matches!(
                                        target,
                                        hir::DefaultAssignTargetV1::Field { .. }
                                    ));
                                    assignments += 1;
                                }
                                _ => {}
                            }
                        }
                        Context::Expression {
                            receiver: Receiver::Member { implicit_this, .. },
                            ..
                        } => {
                            if !implicit_this {
                                explicit += 1;
                            }
                        }
                        Context::Expression {
                            receiver: Receiver::None,
                            ..
                        } => continue,
                    }
                }
            }
            assert!(captures > 0);
            assert!(local_functions > 0);
            assert!(assignments > 0);
            assert!(explicit >= 2);
        });
    });
}
