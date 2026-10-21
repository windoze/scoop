use super::*;
use hir::SemanticTypeStructure as Shape;

mod declaration_binders;

pub(super) fn resolve(
    queries: &mut dyn SemanticQueries,
    context: SemanticContext,
    text: &str,
) -> hir::TypeId {
    queries
        .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
        .unwrap()
}

#[test]
fn descriptions_keep_complete_structural_types_and_actual_generic_declarations() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        let integer = resolve(queries, context, "Int");
        let boolean = resolve(queries, context, "Boolean");
        let tuple = resolve(queries, context, "(Int, Boolean)");
        let Shape::Tuple(elements) = queries
            .describe_type(context.into(), tuple)
            .unwrap()
            .structure
        else {
            panic!("tuple components")
        };
        assert_eq!(elements, [integer, boolean]);
        let function_type = resolve(queries, context, "(Int) -> Boolean");
        let Shape::Function(function_type) = queries
            .describe_type(context.into(), function_type)
            .unwrap()
            .structure
        else {
            panic!("ordinary function signature")
        };
        assert_eq!(function_type.parameter_types, [integer]);
        assert_eq!(function_type.return_type, boolean);
        let pointer = resolve(queries, context, "Ptr<Int>");
        assert!(
            matches!(queries.describe_type(context.into(), pointer).unwrap().structure,
            Shape::Pointer(pointee) if pointee == integer)
        );
        let pointer = resolve(queries, context, "FunPtr<(Int) -> Boolean>");
        let Shape::NativeFunctionPointer(function_type) = queries
            .describe_type(context.into(), pointer)
            .unwrap()
            .structure
        else {
            panic!("native function pointer signature")
        };
        assert_eq!(function_type.parameter_types, [integer]);
        assert_eq!(function_type.return_type, boolean);
        let aliased = resolve(queries, context, "Wrapped<Int>");
        let direct = resolve(queries, context, "Box<Int>");
        assert_eq!(aliased, direct);
        let Shape::Nominal(nominal) = queries
            .describe_type(context.into(), aliased)
            .unwrap()
            .structure
        else {
            panic!("nominal declaration and arguments")
        };
        assert!(matches!(
            nominal.declaration,
            hir::SourceNominalId::GenericTemplate(_)
        ));
        assert_eq!(nominal.kind, hir::StaticNominalKind::Struct);
        assert_eq!(nominal.arguments, [integer]);
        assert_eq!(nominal.parameters[0].name, "T");
        let parameter = resolve(queries, context, "T");
        let Shape::Parameter(parameter) = queries
            .describe_type(context.into(), parameter)
            .unwrap()
            .structure
        else {
            panic!("original binder declaration")
        };
        assert_eq!(
            parameter.id,
            queries.signature(function).type_parameters[0].id
        );
        assert_ne!(parameter.id, nominal.parameters[0].id);
        Ok(Vec::new())
    });
    compile("annotation class Mark\nstruct Box<T>(val value: T)\ntypealias Wrapped<T> = Box<T>\n@Mark fun <T> inspect(value: T) {}", &mut driver).unwrap();
}

#[test]
fn type_parameters_cannot_be_reinterpreted_in_an_unrelated_context() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let functions = marked_functions(queries);
        let first = SemanticContext::Function(functions[0]);
        let second = SemanticContext::Function(functions[1]);
        let ty = resolve(queries, first, "T");
        assert!(queries.describe_type(first.into(), ty).is_ok());
        assert!(queries.describe_type(second.into(), ty).is_err());
        Ok(Vec::new())
    });
    compile("annotation class Mark\n@Mark fun <T> first(value: T) {}\n@Mark fun <T> second(value: T) {}", &mut driver).unwrap();
}

#[test]
fn type_queries_apply_real_parent_and_companion_relations() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let ty = resolve(queries, context, "Box<Int>");
        let description = queries.describe_type(context.into(), ty).unwrap();
        let parent = resolve(queries, context, "View<Int>");
        assert!(description.parents.contains(&parent));
        let Shape::Nominal(nominal) = description.structure else {
            panic!("Box")
        };
        let companion = queries
            .describe_type(context.into(), nominal.companion.unwrap())
            .unwrap();
        let Shape::Nominal(companion) = companion.structure else {
            panic!("companion")
        };
        assert_eq!(companion.kind, hir::StaticNominalKind::Object);
        assert_eq!(companion.arguments, nominal.arguments);
        assert_ne!(companion.declaration, nominal.declaration);
        Ok(Vec::new())
    });
    compile("annotation class Mark\ninterface View<T> {}\nstruct Box<T>(val value: T) : View<T> { private companion object Factory {} }\n@Mark fun inspect() {}", &mut driver).unwrap();
}

#[test]
fn failed_type_queries_validate_annotations_and_do_not_add_program_uses() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let count = queries.annotations().type_uses.len();
        for text in ["Storage<String>", "Callback<String>"] {
            let errors = queries
                .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                .unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("not GC-free"))
            );
        }
        let errors = queries
            .resolve_type(
                context,
                &scoop_parser::parse_type("@NoSuchAnnotation Int").unwrap(),
            )
            .unwrap_err();
        assert!(!errors.is_empty());
        let valid = resolve(queries, context, "Storage<Int>");
        assert!(queries.describe_type(context.into(), valid).is_ok());
        assert_eq!(queries.annotations().type_uses.len(), count);
        Ok(Vec::new())
    });
    let module = compile(
        "annotation class Mark\n@NoGC struct Storage<T>(val value: T)\ntypealias Callback<T> = () -> Storage<T>\n@Mark fun inspect() {}",
        &mut driver,
    )
    .unwrap();
    let (storage, _) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Storage")
        .unwrap();
    let storage = module.nominal_identities[storage].declaration_id();
    assert!(module.struct_applications.values().all(|application| {
        application.template != storage
            || application
                .arguments
                .iter()
                .all(|argument| matches!(module.types[*argument], hir::Type::Param(_)))
    }));
}
