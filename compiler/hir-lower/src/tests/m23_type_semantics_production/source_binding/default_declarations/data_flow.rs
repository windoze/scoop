use super::*;
use hir::{DefaultExpressionKindV1 as Expr, DefaultStatementKindV1 as Stmt};

pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/data-flow.scoop"
));
mod fields;

fn body(t: &Template, statements: Vec<hir::DefaultStatementV1>) -> Template {
    Template::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        t.locals().clone(),
        hir::ExportDefaultBodyV1::try_new(statements, t.body().value().clone()).unwrap(),
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        t.references().clone(),
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap()
}
fn read(t: &Template, selector: LocalValueSelector) -> hir::DefaultStatementV1 {
    let ty = t.locals().get(&selector).unwrap().value_type().clone();
    hir::DefaultStatementV1::try_new(
        Stmt::Expr(Box::new(
            hir::DefaultExpressionV1::try_new(
                Expr::Local(selector),
                ty,
                t.definition_origin().clone(),
            )
            .unwrap(),
        )),
        t.definition_origin().clone(),
    )
    .unwrap()
}

fn assign_parameter(t: &Template) -> hir::DefaultStatementV1 {
    let local = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let source = read(t, local.clone());
    let Stmt::Expr(value) = source.kind() else {
        panic!("read creates an expression statement");
    };
    hir::DefaultStatementV1::try_new(
        Stmt::Assign {
            target: Box::new(hir::DefaultAssignTargetV1::Local { local }),
            value: value.clone(),
        },
        t.definition_origin().clone(),
    )
    .unwrap()
}

#[test]
fn default_source_data_flow_rejects_future_reads_immutable_writes_and_escaping_breaks() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table.get(key(output, "FlowHost.scoped", 1)).unwrap();
        let selector = original
            .locals()
            .records()
            .iter()
            .find(|local| {
                matches!(
                    local.selector(),
                    LocalValueSelector::LocalDeclaration { .. }
                )
            })
            .unwrap()
            .selector()
            .clone();
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for (index, first) in [
                read(original, selector.clone()),
                hir::DefaultStatementV1::try_new(Stmt::Break, original.definition_origin().clone())
                    .unwrap(),
                assign_parameter(original),
            ]
            .into_iter()
            .enumerate()
            {
                let mut statements = vec![first];
                statements.extend_from_slice(original.body().statements());
                let changed = replace(&table, body(original, statements));
                let Error::Record { key: failed, error } = parameters
                    .bind_default_declarations(&changed, &[], &mut meter())
                    .unwrap_err()
                else {
                    panic!("expected a data-flow failure");
                };
                assert_eq!(failed, original.key());
                let Error::DataFlow(error) = *error else {
                    panic!("expected source data-flow error");
                };
                let description = format!("case {index}: {error:?}");
                assert!(match (index, *error) {
                    (
                        0,
                        hir::ExportDefaultLocalDataFlowValidationError::Local {
                            site: hir::DefaultLocalDataFlowSiteV1::Expression,
                            selector: actual,
                            error: hir::DefaultLocalDataFlowLocalError::UseBeforeDefinition,
                        },
                    ) => *actual == selector,
                    (
                        1,
                        hir::ExportDefaultLocalDataFlowValidationError::LoopControlOutsideLoop {
                            control: hir::DefaultLoopControlV1::Break,
                        },
                    ) => true,
                    (
                        2,
                        hir::ExportDefaultLocalDataFlowValidationError::Local {
                            site: hir::DefaultLocalDataFlowSiteV1::Assignment,
                            selector: actual,
                            error:
                                hir::DefaultLocalDataFlowLocalError::Mutability {
                                    expected: hir::CanonicalBooleanV1::False,
                                    actual: hir::CanonicalBooleanV1::True,
                                },
                        },
                    ) =>
                        *actual
                            == LocalValueSelector::Parameter {
                                declaration_index: 0
                            },
                    _ => false,
                }, "{description}");
            }
        });
    });
}
