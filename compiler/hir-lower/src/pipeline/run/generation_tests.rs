use super::*;
use hir::{MethodBodyGenerator, SemanticContext, SemanticQueries};

mod queries;

struct Generator<F>(F);

impl<F> MethodBodyGenerator for Generator<F>
where
    F: FnMut(&mut dyn SemanticQueries) -> Result<Vec<hir::GeneratedMethodBody>, Vec<Diagnostic>>,
{
    fn generate(
        &mut self,
        queries: &mut dyn SemanticQueries,
    ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<Diagnostic>> {
        self.0(queries)
    }
}

fn compile(
    source: &str,
    generator: &mut dyn MethodBodyGenerator,
) -> Result<hir::Module, Vec<Diagnostic>> {
    let core = crate::tests::core_file();
    let file = scoop_parser::parse(source).unwrap();
    let sources = vec![
        SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(0),
            kind: SourceKind::Core,
            role: scoop_identity::SourceRole::Production,
            identity: crate::tests::core_source_identity("src/core.scoop"),
            name: "core.scoop".into(),
            source: String::new(),
        },
        SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(1),
            kind: SourceKind::CurrentUnit,
            role: scoop_identity::SourceRole::Production,
            identity: crate::tests::test_source_identity("src/user.scoop"),
            name: "user.scoop".into(),
            source: source.into(),
        },
    ];
    let lowerer =
        Lowerer::new().with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly);
    let dependencies = hir::ImportedDependencySelectionPlan::empty(lowerer.current_cone());
    lowerer
        .with_imported_dependencies(dependencies)
        .run(&[core, file], None, Some(generator))
        .map(|(module, _, _)| module)
}

fn generated(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    source: &str,
) -> hir::GeneratedMethodBody {
    let identity = queries.method_identity(function).unwrap();
    let name = match identity {
        hir::HirSourceFunctionIdentity::Plain(record) => record.id().to_string(),
        hir::HirSourceFunctionIdentity::Generic(record) => record.id().to_string(),
    };
    hir::GeneratedMethodBody {
        function,
        body: scoop_parser::parse_function_body(source).unwrap(),
        source: hir::GeneratedMethodSource {
            identity: crate::tests::test_source_identity(&format!("generated/{name}.scoop")),
            name: format!("generated/{name}.scoop"),
            text: source.to_owned().into(),
        },
        origin: queries.signature(function).origin,
    }
}

#[test]
fn missing_method_keeps_one_signature_and_all_real_requirements() {
    let mut queried_identity = None;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let pending = queries.pending_methods();
        assert_eq!(pending.len(), 1);
        let pending = &pending[0];
        assert_eq!(pending.requirements.len(), 2);
        let function = pending.function;
        let signature = queries.signature(function);
        assert_eq!(signature.name, "echo");
        assert_eq!(signature.parameters[0].name, "value");
        assert_eq!(signature.type_parameters[0].name, "T");
        assert_eq!(signature.owner_parameter_count, 1);
        let context = SemanticContext::Function(function);
        let ty = queries
            .resolve_type(context, &scoop_parser::parse_type("T").unwrap())
            .unwrap();
        assert!(queries.type_relation(
            context,
            hir::SemanticTypeRelation::Equal,
            ty,
            signature.result
        ));
        assert_eq!(
            queries.type_signature(context, ty).unwrap(),
            scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
        );
        assert_eq!(queries.describe_type(context, ty).name, "T");
        queried_identity = Some((function, queries.method_identity(function).unwrap()));
        Ok(vec![generated(queries, function, "{ return value }")])
    });
    let module = compile(
        "interface Left<T> { fun echo(value: T): T }\n\
         interface Right<T> { fun echo(value: T): T }\n\
         struct Echo<T>() : Left<T>, Right<T>\n\
         fun use(value: Echo<Int>): Int = value.echo(42)",
        &mut driver,
    )
    .unwrap();
    let (function, identity) = queried_identity.unwrap();
    assert!(
        matches!(&module.function_identities[function], hir::HirFunctionIdentity::Source(actual) if actual.declaration() == identity.declaration())
    );
    let hir::FunctionKind::User(body) = &module.functions[function].kind else {
        panic!("ordinary generated body")
    };
    assert_eq!(module.functions[function].params[1].name, "value");
    let hir::StatementKind::Return { value } = &body.statements[0].kind else {
        panic!("generated return")
    };
    assert_eq!(
        module.source_files[value.origin.definition().file as usize].source,
        "{ return value }"
    );
    let hir::LocalValueDefinitionSite::Source(parameter) =
        body.locals[module.functions[function].params[1].local].definition
    else {
        panic!("source parameter origin")
    };
    assert_eq!(
        module.source_files[parameter.file as usize].name,
        "user.scoop"
    );
}

#[test]
fn generated_block_uses_original_names_and_retains_local_callable_sources() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let method = queries.pending_methods()[0].function;
        Ok(vec![generated(
            queries,
            method,
            "{ fun local(value: Int): Int = pass(value)\n return local(7) }",
        )])
    });
    let module = compile(
        "private fun pass(value: Int): Int = value\n\
         interface Result { fun result(): Int }\n\
         class Value : Result\n\
         fun use(value: Value): Int = value.result()",
        &mut driver,
    )
    .unwrap();
    assert_eq!(module.local_functions.len(), 1);
    let local = module.local_functions.values().next().unwrap();
    assert!(
        module.source_files[local.origin.file as usize]
            .name
            .starts_with("generated/")
    );
}

#[test]
fn generated_local_types_aliases_and_lambdas_keep_fragment_origins() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let method = queries.pending_methods()[0].function;
        Ok(vec![generated(
            queries,
            method,
            "{ typealias Number = Int\n struct Item(val value: Number)\n val get: () -> Int = { Item(9).value }\n return get() }",
        )])
    });
    let module = compile(
        "interface Result { fun result(): Int }\nclass Value : Result",
        &mut driver,
    )
    .unwrap();
    assert_eq!(module.lambdas.len(), 1);
    let alias = module
        .type_aliases
        .values()
        .find(|alias| alias.name == "Number")
        .unwrap();
    assert!(
        module.source_files[alias.origin.file as usize]
            .name
            .starts_with("generated/")
    );
}

#[test]
fn inherited_pending_methods_do_not_mix_another_owners_type_arguments() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let pending = queries.pending_methods();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].requirements.len(), 1);
        let method = pending[0].function;
        Ok(vec![generated(queries, method, "{ return value }")])
    });
    compile(
        "interface I<T> { fun echo(value: T): T }\n\
         interface J { fun echo(value: Int): Int }\n\
         open class Base<T> : I<T>\n\
         class Child : Base<Int>(), J",
        &mut driver,
    )
    .unwrap();
}

#[test]
fn duplicate_returned_body_cannot_overwrite_a_completed_implementation() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = queries.pending_methods()[0].function;
        let body = generated(queries, function, "{ return 1 }");
        Ok(vec![body.clone(), body])
    });
    let errors = compile("interface I { fun read(): Int }\nclass C : I", &mut driver).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("remaining implementation method")),
        "{errors:?}"
    );
}

#[test]
fn ordinary_impl_defaults_and_abstract_slots_are_excluded() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        assert!(queries.pending_methods().is_empty());
        Ok(Vec::new())
    });
    compile(
        "interface Value { fun value(): Int }\n\
         interface DefaultValue : Value { override fun value(): Int = 1 }\n\
         class Explicit : Value { override fun value(): Int = 2 }\n\
         class Default : DefaultValue\n\
         abstract class Abstract : Value",
        &mut driver,
    )
    .unwrap();
}

#[test]
fn omitted_body_is_still_an_ordinary_missing_implementation_error() {
    let mut driver = Generator(|_: &mut dyn SemanticQueries| Ok(Vec::new()));
    let errors = compile("interface I { fun read(): Int }\nclass C : I", &mut driver).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("does not implement interface method `I.read`")),
        "{errors:?}"
    );
}

#[test]
fn generated_type_errors_point_into_fragment_and_keep_request_note() {
    let fragment = "{\n    val 中文 = true\n    return 中文\n}";
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let method = queries.pending_methods()[0].function;
        Ok(vec![generated(queries, method, fragment)])
    });
    let errors = compile("interface I { fun read(): Int }\nclass C : I", &mut driver).unwrap_err();
    let error = errors
        .iter()
        .find(|error| error.message.contains("Int") && error.message.contains("Boolean"))
        .expect("ordinary result type diagnostic");
    assert!(
        error
            .source
            .as_ref()
            .unwrap()
            .logical_path()
            .as_str()
            .starts_with("generated/")
    );
    assert!(error.span.unwrap().end as usize <= fragment.len());
    assert_eq!(
        error.notes.last().unwrap().source.as_deref(),
        Some(&crate::tests::test_source_identity("src/user.scoop"))
    );
}
