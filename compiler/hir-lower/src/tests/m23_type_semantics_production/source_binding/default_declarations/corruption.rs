use super::*;
use scoop_identity::{
    NonEmptyVec, StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

#[derive(Clone, Copy, Debug)]
enum Change {
    Path,
    NestedPath,
    Mapping,
    Prefix,
    Result,
    Receiver,
    Suspend,
}
fn corrupt(t: &Template, change: Change) -> Template {
    let wrong = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let mut locals = t.locals().records().to_vec();
    let mut result = t.result().clone();
    let mut body = t.body().clone();
    let mut receiver = t.receiver().clone();
    let mut mapping = t.type_parameters().clone();
    let mut path = t.definition_path().clone();
    let mut suspend = t.allows_suspend();
    let mut replace_local = |selector: LocalValueSelector, value_type: SignatureTypeKey| {
        let record = locals
            .iter_mut()
            .find(|l| l.selector() == &selector)
            .unwrap();
        *record = hir::TemplateLocalRecordV1::try_new(
            selector,
            value_type,
            record.mutable(),
            record.definition().clone(),
        )
        .unwrap();
    };
    match change {
        Change::Path => {
            path = StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 9),
                [],
            )
        }
        Change::NestedPath => {
            path = StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::LocalDeclaration, 0),
                path.segments().iter().copied(),
            );
        }
        Change::Mapping => {
            let mut arguments = mapping.arguments().to_vec();
            arguments[0] = wrong;
            mapping = hir::CanonicalBinderUseListV1::try_new(arguments).unwrap();
        }
        Change::Prefix => replace_local(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
            wrong,
        ),
        Change::Result => {
            result = wrong;
            body = hir::ExportDefaultBodyV1::try_new(
                t.body().statements().to_vec(),
                hir::DefaultExpressionV1::try_new(
                    t.body().value().kind().clone(),
                    result.clone(),
                    t.body().value().definition_origin().clone(),
                )
                .unwrap(),
            )
            .unwrap();
        }
        Change::Receiver => {
            let SignatureTypeKey::NominalApplication { origin, .. } =
                t.receiver().receiver().unwrap().value_type()
            else {
                panic!("generic provider receiver");
            };
            let value_type = SignatureTypeKey::NominalApplication {
                origin: *origin,
                arguments: NonEmptyVec::from_first(
                    wrong,
                    [SignatureTypeKey::Binder { depth: 0, index: 0 }],
                ),
            };
            replace_local(LocalValueSelector::This, value_type.clone());
            receiver = hir::OptionalTemplateReceiverV1::Present(
                hir::TemplateReceiverV1::try_new(LocalValueSelector::This, value_type).unwrap(),
            );
        }
        Change::Suspend => suspend = hir::CanonicalBooleanV1::True,
    }
    Template::try_new(
        t.key(),
        t.definition_root(),
        path,
        hir::CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        body,
        result,
        suspend,
        mapping,
        receiver,
        t.value_parameters().clone(),
        t.references().clone(),
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap()
}
#[test]
fn bound_default_declarations_reject_raw_types_even_when_substitution_collapses_them() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let inherited = table.get(key(output, "ContractChild.pick", 2)).unwrap();
        assert_eq!(
            inherited.type_parameters().arguments(),
            &[
                SignatureTypeKey::Binder { depth: 0, index: 0 },
                SignatureTypeKey::Binder { depth: 0, index: 0 }
            ]
        );
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for change in [
                Change::Path,
                Change::NestedPath,
                Change::Mapping,
                Change::Prefix,
                Change::Result,
                Change::Receiver,
                Change::Suspend,
            ] {
                let original = if matches!(change, Change::Mapping) {
                    table.get(key(output, "ContractBase.pick", 2)).unwrap()
                } else {
                    inherited
                };
                let changed = replace(&table, corrupt(original, change));
                let Error::Record { key: failed, error } = parameters
                    .bind_default_declarations(&changed, &[], &mut meter())
                    .unwrap_err()
                else {
                    panic!("expected a declaration contract failure for {change:?}");
                };
                assert_eq!(failed, original.key());
                assert!(match (change, *error) {
                    (Change::Path, Error::DefinitionPath)
                    | (Change::NestedPath, Error::DefinitionPath)
                    | (Change::Mapping, Error::DirectMapping { index: 0 })
                    | (
                        Change::Prefix,
                        Error::Prefix(
                            hir::MeteredTemplateValueParameterSemanticValidationError::LocalType {
                                position: 0,
                            },
                        ),
                    )
                    | (Change::Result, Error::ResultType)
                    | (
                        Change::Receiver,
                        Error::Receiver(
                            hir::MeteredTemplateReceiverSemanticValidationError::CallableType,
                        ),
                    )
                    | (Change::Suspend, Error::SuspendPermission) => true,
                    (_, error) => panic!("unexpected {change:?}: {error:?}"),
                });
            }
        });
    });
}

#[test]
fn nominal_default_paths_cannot_claim_a_nested_lexical_root() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for original in table.records() {
                let changed = replace(&table, corrupt(original, Change::NestedPath));
                let Error::Record { key, error } = parameters
                    .bind_default_declarations(&changed, &[], &mut meter())
                    .unwrap_err()
                else {
                    panic!("expected nominal default path failure");
                };
                assert_eq!(key, original.key());
                assert!(matches!(*error, Error::DefinitionPath), "{error:?}");
            }
        });
    });
}
