use super::*;
use hir::concrete;
use std::collections::HashSet;

pub(super) fn check_parameter_aliases(output: &hir::Output, name: &str, count: usize) {
    let module = &output.local;
    let providers = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == name)
        .collect::<Vec<_>>();
    assert_eq!(providers.len(), count);
    let mut captured = HashSet::new();
    for (function_id, function) in module.functions.iter() {
        for capture in &function.capture_parameters {
            let value = module
                .local_value_identities
                .function_local(function_id, capture.local);
            if let Some((provider_id, provider)) = providers
                .iter()
                .find(|(_, provider)| provider.materialization == value.key().owner())
            {
                let parameter = module
                    .local_value_identities
                    .function_local(*provider_id, provider.params[0].local);
                assert_eq!(value, parameter);
                assert_eq!(function.params[0].ty, provider.params[0].ty);
                captured.insert(value.id());
            }
        }
    }
    assert_eq!(captured.len(), count);
}

pub(super) fn check_default_local_and_expansion_values(output: &hir::Output) {
    let source = output
        .export
        .export_default_exprs
        .iter()
        .find_map(|(_, body)| {
            body.locals
                .iter()
                .find(|(_, local)| local.name == "result")
                .map(|(_, local)| local)
        })
        .unwrap();
    let hir::LocalValueDefinitionSite::Source(origin) = source.definition else {
        panic!("source local");
    };
    let module = &output.local;
    let (provider_id, provider) = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "capturedCallback")
        .unwrap();
    let concrete::FunctionKind::User(provider_body) = &provider.kind else {
        panic!("source body");
    };
    assert!(
        provider_body
            .locals
            .iter()
            .all(|(_, local)| local.name != "result")
    );
    let (main, _) = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "main")
        .unwrap();
    let mut identities = Vec::new();
    let mut evaluated = HashSet::new();
    for (id, lambda) in module.lambdas.iter() {
        for (index, capture) in lambda.captures.iter().enumerate() {
            if capture.binding.into_raw() != source.binding.into_raw() {
                continue;
            }
            let value = module.local_value_identities.lambda_capture(id, index);
            assert_eq!(
                value.key().owner(),
                module.functions[provider_id].materialization
            );
            assert_eq!(value.key().selector(), &source.selector);
            let definition = module
                .local_value_identities
                .definition_origins()
                .get(value.id())
                .unwrap()
                .origin();
            assert_eq!(
                (definition.span().start_byte(), definition.span().end_byte()),
                (u64::from(origin.span.start), u64::from(origin.span.end))
            );
            identities.push(value.id());
            let concrete::ExprKind::Local(local) = capture.source.kind else {
                panic!("expanded local read");
            };
            let actual = module.local_value_identities.function_local(main, local);
            assert_ne!(value.id(), actual.id());
            evaluated.insert(actual.id());
        }
    }
    assert_eq!(identities.len(), 2);
    assert_eq!(identities[0], identities[1]);
    assert_eq!(
        evaluated.len(),
        2,
        "each expansion evaluates into its own storage"
    );
}

pub(super) fn check_unused_default_is_not_a_materialization_root(output: &hir::Output) {
    let (provider, _) = output
        .export
        .functions
        .iter()
        .find(|(_, f)| f.name == "unused")
        .unwrap();
    let functions = output
        .export
        .lambdas
        .iter()
        .filter(|(_, f)| f.definition_root == hir::LexicalDefinitionRoot::Function(provider))
        .map(|(_, f)| f.function)
        .chain(output.export.local_functions.iter().filter_map(|(_, f)| {
            let (function, root) = f.source()?;
            (root == hir::LexicalDefinitionRoot::Function(provider)).then_some(function)
        }))
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);
    for function in functions {
        let source = &output.export.functions[function];
        assert!(
            !output
                .local
                .functions
                .iter()
                .any(|(_, concrete)| concrete.name == source.name)
        );
    }
}
