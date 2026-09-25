use super::*;
use hir::{
    DefaultBodyProviderTypeSiteV1 as Site, DefaultSourceNestedCallableDescriptorV1 as Descriptor,
    SignatureBinderScopeError as ScopeError,
};

#[test]
fn source_local_binders_reject_signature_and_capture_corruption_using_canonical_arity() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table.get(key(output, "LocalGenericHost.pick", 1)).unwrap();
        let index = original
            .index_nested_callables(&scoop_wire::WirePath::root())
            .unwrap();
        let occurrence = &index.occurrences()[0];
        let Descriptor::LocalFunction(local) = occurrence.descriptor() else {
            panic!("expected local function")
        };
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            for case in 0..3 {
                let (changed_local, site, expected) = corrupt(local, case);
                let mut statements = original.body().statements().to_vec();
                statements.insert(
                    0,
                    hir::DefaultStatementV1::try_new(
                        hir::DefaultStatementKindV1::LocalFunction(changed_local),
                        occurrence.definition_origin().clone(),
                    )
                    .unwrap(),
                );
                let body =
                    hir::ExportDefaultBodyV1::try_new(statements, original.body().value().clone())
                        .unwrap();
                let changed = replace(
                    &table,
                    envelope::rebuild(original, original.locals().records().to_vec(), body),
                );
                let error = parameters
                    .bind_default_declarations(&changed, &[])
                    .unwrap_err();
                assert_scope_error(error, original.key(), site, expected);
            }
        });
    });
}

fn corrupt(
    local: &hir::DefaultLocalFunctionV1,
    case: u8,
) -> (hir::DefaultLocalFunctionV1, Site, ScopeError) {
    let mut signature = local.function_type().clone();
    let mut captures = local.captures().to_vec();
    let SignatureTypeKey::Function {
        parameters, result, ..
    } = &mut signature
    else {
        panic!("function signature")
    };
    let (site, expected) = match case {
        0 => {
            parameters[0] = SignatureTypeKey::Binder { depth: 0, index: 1 };
            (
                Site::NestedCallableFunction,
                ScopeError::IndexOutOfRange {
                    depth: 0,
                    index: 1,
                    arity: 1,
                },
            )
        }
        1 => {
            **result = SignatureTypeKey::Binder { depth: 2, index: 0 };
            (
                Site::NestedCallableFunction,
                ScopeError::DepthOutOfRange {
                    depth: 2,
                    available_depths: 2,
                },
            )
        }
        _ => {
            let capture = &captures[0];
            captures[0] = hir::DefaultCaptureV1::new(
                capture.source().clone(),
                SignatureTypeKey::Binder { depth: 1, index: 0 },
                capture.first_use_origin().clone(),
            );
            (
                Site::CaptureValue,
                ScopeError::DepthOutOfRange {
                    depth: 1,
                    available_depths: 1,
                },
            )
        }
    };
    let changed = hir::DefaultLocalFunctionV1::try_new(
        local.declaration(),
        local.definition_path().clone(),
        signature,
        captures,
        100,
    )
    .unwrap();
    (changed, site, expected)
}

fn assert_scope_error(
    error: Error,
    expected_key: hir::ProtectedDefaultTemplateKeyV1,
    expected_site: Site,
    expected_scope: ScopeError,
) {
    let Error::Record { key, error } = error else {
        panic!("expected record error")
    };
    assert_eq!(key, expected_key);
    let Error::BodyEnvelope(error) = *error else {
        panic!("expected body envelope error: {error:?}")
    };
    let hir::DefaultBodyProviderEnvelopeSemanticValidationError::Type { site, error, .. } = *error
    else {
        panic!("expected type error")
    };
    assert_eq!(site, expected_site);
    assert!(
        matches!(*error, hir::SignatureTypeSemanticError::BinderScope(actual)
        if actual == expected_scope)
    );
}
