use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

mod accessors;
mod bodies;
mod callable_argument_contexts;
mod callable_arguments;
mod callable_literal_contexts;
mod callable_literals;
mod callable_values;
mod callables;
mod calls;
mod candidates;
mod candidates_imported;
mod conditionals;
mod constructors;
mod definitions;
mod do_while;
mod documentation;
mod exception_flow;
mod exceptions;
mod file_headers;
mod imported_signatures;
mod imports;
mod iteration;
mod local_function_contexts;
mod local_functions;
mod loops;
mod member_implementations;
mod members;
mod members_imported;
mod named_arguments;
mod operands;
mod properties;
mod properties_imported;
mod property_declarations;
mod qualified_alias_candidates;
mod qualified_candidates;
mod qualified_imported_candidates;
mod receivers;
mod recovered_classes;
mod recovered_enums;
mod recovered_interfaces;
mod recovered_objects;
mod recovered_structs;
mod references;
mod runtime_initializers;
mod selectors;
mod trailing_callables;
mod type_definitions;
mod type_descriptions;
mod type_implementations;
mod types;
mod unavailable_file_names;
mod when;
mod when_inputs;

fn analyze(text: &str) -> DeclarationAnalysis {
    analyze_cancellable(text, || false).unwrap()
}

fn analyze_cancellable(
    text: &str,
    is_cancelled: impl Fn() -> bool,
) -> Result<DeclarationAnalysis, EditingAnalysisError> {
    let core = crate::tests::complete_core_file();
    let core_identity = crate::tests::core_source_identity("src/core.scoop");
    let identity = crate::tests::core_source_identity("src/editing.scoop");
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let inputs = [
        ast::DeclarationSource {
            identity: &core_identity,
            role: scoop_identity::SourceRole::Production,
            text: "",
            header: core.header().into(),
            documentation: None,
            declarations: core
                .declarations
                .iter()
                .map(ast::DeclarationInput::Complete)
                .collect(),
        },
        scoop_parser::declaration_source(
            &parsed,
            &identity,
            scoop_identity::SourceRole::Production,
            text,
        ),
    ];
    let world =
        hir::ImportedSemanticWorld::from_dependencies(ConeIdentity::CORE, Vec::new(), Vec::new())
            .unwrap();
    analyze_declarations(
        &inputs,
        &CoreProtocolInput::CurrentDeclarations,
        &world,
        &[],
        is_cancelled,
    )
}

fn function(analysis: &DeclarationAnalysis, name: &str) -> hir::FunctionId {
    analysis
        .functions()
        .iter()
        .find(|function| function.name == name)
        .unwrap_or_else(|| panic!("missing function `{name}`"))
        .function
}

#[test]
fn cancelled_declarations_are_distinct_from_source_errors_and_can_be_retried() {
    let text = "fun healthy(value: Boolean): Boolean = value\nfun damaged(value: Missing) {}";
    let checks = std::cell::Cell::new(0);
    let result = analyze_cancellable(text, || {
        checks.set(checks.get() + 1);
        checks.get() == 8
    });
    assert!(matches!(result, Err(EditingAnalysisError::Cancelled)));
    assert_eq!(checks.get(), 8);
    let analysis = analyze(text);
    assert!(matches!(
        analysis.signature(function(&analysis, "healthy")),
        V::Known(_)
    ));
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|error| error.message.contains("Missing"))
    );
}

#[test]
fn recovered_signatures_keep_parameter_types_and_distinguish_pending_results() {
    let text = "fun editing(value: Boolean): Boolean { value. }\nfun healthy(value: Boolean): Boolean = value\nfun inferred(value: Boolean) =";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    for (name, pending) in [("editing", false), ("inferred", true), ("healthy", false)] {
        let function = function(&analysis, name);
        let V::Known(signature) = analysis.signature(function) else {
            panic!("signature of {name} was not preserved");
        };
        assert_eq!(signature.parameters[0].name, "value");
        assert_eq!(
            matches!(signature.result, V::Unavailable(U::PendingInference)),
            pending
        );
        let value_type = signature.parameters[0].value_type;
        let V::Known(ty) = analysis.describe_type(function, value_type).unwrap() else {
            panic!("a known parameter must keep its type even when the result is pending");
        };
        assert_eq!(ty.name, "Boolean");
        let site = analysis
            .functions()
            .iter()
            .find(|site| site.function == function)
            .unwrap();
        assert_eq!(
            &text[site.name_origin.span.start as usize..site.name_origin.span.end as usize],
            name
        );
        assert_eq!(
            site.name_origin.source.logical_path().as_str(),
            "src/editing.scoop"
        );
    }
}

#[test]
fn generic_type_descriptions_use_the_same_declaration_context_as_fixed_queries() {
    let mut analysis = analyze("fun <T> generic(value: T): T { value. }");
    let function = function(&analysis, "generic");
    let V::Known(signature) = analysis.signature(function) else {
        panic!("complete generic signature");
    };
    let V::Known(result) = signature.result else {
        panic!("explicit generic result");
    };
    let V::Known(description) = analysis.describe_type(function, result).unwrap() else {
        panic!("known result type");
    };
    assert!(
        matches!(description.structure, hir::SemanticTypeStructure::Parameter(parameter) if parameter.name == "T")
    );
}

#[test]
fn failed_signatures_are_unavailable_beside_independent_known_signatures() {
    let analysis = analyze(
        "fun damaged(value: Missing): Boolean = true\nfun healthy(value: Boolean): Boolean = value",
    );
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Missing"))
    );
    assert!(matches!(
        analysis.signature(function(&analysis, "damaged")),
        V::Unavailable(U::SignatureFailure)
    ));
    assert!(matches!(
        analysis.signature(function(&analysis, "healthy")),
        V::Known(_)
    ));
}

#[test]
fn declared_defaults_and_vararg_types_do_not_require_default_body_preparation() {
    let text = "fun defaults(value: Boolean = missing, vararg rest: Boolean = missing): Boolean { value. }";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let function = function(&analysis, "defaults");
    let V::Known(signature) = analysis.signature(function) else {
        panic!("parameter declarations are complete before checking their defaults");
    };
    assert!(signature.parameters[0].vararg_element_type.is_none());
    let element = signature.parameters[1].vararg_element_type.unwrap();
    let V::Known(description) = analysis.describe_type(function, element).unwrap() else {
        panic!("vararg element type is known");
    };
    assert_eq!(description.name, "Boolean");
    for parameter in signature.parameters {
        let default = parameter.declared_default.unwrap();
        assert_eq!(
            &text[default.start as usize..default.end as usize],
            "missing"
        );
    }
}
