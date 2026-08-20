use super::*;

#[test]
fn function_types_own_unique_canonical_types_in_both_hir_products() {
    let ordinary = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let suspended = ty_function(true, vec![ty_named("Int")], ty_named("String"));
    let output = lower_user_output(file(vec![
        fun_sig(
            "useFunctions",
            vec![],
            vec![
                ("first", ordinary.clone()),
                ("second", ordinary),
                ("suspended", suspended),
            ],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("canonical function types must lower");

    for (function_type, signature) in output.export.function_types.iter() {
        assert!(matches!(
            output.export.types[signature.canonical_type],
            hir::Type::Function(found) if found == function_type
        ));
        assert_eq!(
            output
                .export
                .types
                .iter()
                .filter(
                    |(_, ty)| matches!(ty, hir::Type::Function(found) if *found == function_type)
                )
                .count(),
            1
        );
    }

    for (function_type, signature) in output.local.function_types.iter() {
        assert!(matches!(
            &output.local.types[signature.canonical_type].kind,
            hir::concrete::TypeKind::Function(found) if *found == function_type
        ));
        assert_eq!(
            output
                .local
                .types
                .iter()
                .filter(|(_, ty)| matches!(&ty.kind, hir::concrete::TypeKind::Function(found) if *found == function_type))
                .count(),
            1
        );
    }

    let export_use = output
        .export
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "useFunctions")
        .expect("export function");
    assert_eq!(export_use.params[0].ty, export_use.params[1].ty);
    assert_ne!(export_use.params[0].ty, export_use.params[2].ty);

    let local_use = output
        .local
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "useFunctions")
        .expect("local function");
    assert_eq!(local_use.params[0].ty, local_use.params[1].ty);
    assert_ne!(local_use.params[0].ty, local_use.params[2].ty);
}

#[test]
fn compiler_exception_core_is_complete_in_export_and_local_hir() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![
            val("defaultException", call("IllegalStateException", vec![])),
            val(
                "cycleException",
                call(
                    "IllegalStateException",
                    vec![some(str_lit("initialization cycle: sample.value"))],
                ),
            ),
        ],
    )]))
    .expect("defaulted and explicit compiler exception construction must lower");

    let export = defined_export_core(output.export.module()).exceptions;
    let export_names = [
        export.throwable,
        export.unwrap_exception,
        export.class_cast_exception,
        export.arithmetic_exception,
        export.index_out_of_bounds_exception,
        export.illegal_state_exception,
        export.illegal_argument_exception,
    ]
    .map(|exception| output.export.classes[exception.class()].name.as_str());
    assert_eq!(
        export_names,
        [
            "Throwable",
            "UnwrapException",
            "ClassCastException",
            "ArithmeticException",
            "IndexOutOfBoundsException",
            "IllegalStateException",
            "IllegalArgumentException",
        ]
    );

    let local = defined_concrete_core(&output.local).exceptions;
    let local_exceptions = [
        local.throwable,
        local.unwrap_exception,
        local.class_cast_exception,
        local.arithmetic_exception,
        local.index_out_of_bounds_exception,
        local.illegal_state_exception,
        local.illegal_argument_exception,
    ];
    let local_names =
        local_exceptions.map(|exception| output.local.classes[exception.class()].name.as_str());
    assert_eq!(local_names, export_names);
    for exception in local_exceptions {
        let constructor = &output.local.class_constructors[exception.callable()];
        assert_eq!(constructor.class, exception.class());
        assert!(constructor.parameters.is_empty());
    }

    let illegal_state = export.illegal_state_exception.class();
    let source_constructor = output.export.classes[illegal_state]
        .constructors
        .iter()
        .copied()
        .find(|constructor| {
            output.export.class_constructors[*constructor]
                .parameters
                .len()
                == 1
        })
        .expect("IllegalStateException retains its message source constructor");
    let parameter_interface = output
        .export
        .source_parameter_interfaces
        .iter()
        .find(|interface| {
            interface.owner == hir::ExportParameterOwner::ClassConstructor(source_constructor)
        })
        .expect("the source constructor exports its default protocol");
    let hir::ExportParameterCalling::Default { source, .. } =
        parameter_interface.parameters[0].calling
    else {
        panic!("compiler exception source constructor default");
    };
    let expression = output.export.export_default_sources[source]
        .declared()
        .unwrap()
        .0;
    assert_eq!(
        output.export.export_default_exprs[expression].definition_root,
        hir::LexicalDefinitionRoot::ClassConstructor(source_constructor)
    );
    assert!(
        output.export.class_constructors[export.illegal_state_exception.callable()]
            .parameters
            .is_empty(),
        "compiler control flow receives a physical zero-parameter adapter"
    );

    let export_cycle_thrower = &output.export.functions[export.initialization_cycle_thrower];
    assert_eq!(export_cycle_thrower.name, "__scoopThrowInitializationCycle");
    assert_eq!(
        export_cycle_thrower.access.declared,
        hir::DeclaredVisibility::Internal
    );
    assert_eq!(export_cycle_thrower.params.len(), 1);
    assert_eq!(export_cycle_thrower.params[0].ty, output.export.string);
    assert_eq!(export_cycle_thrower.return_ty, output.export.unit);

    let local_cycle_thrower = &output.local.functions[local.initialization_cycle_thrower];
    assert_eq!(local_cycle_thrower.name, export_cycle_thrower.name);
    assert_eq!(local_cycle_thrower.params.len(), 1);
    assert_eq!(local_cycle_thrower.params[0].ty, output.local.string);
    assert_eq!(local_cycle_thrower.return_ty, output.local.unit);
}

#[test]
fn compiler_exception_core_rejects_a_missing_or_public_initialization_cycle_thrower() {
    const NAME: &str = "__scoopThrowInitializationCycle";

    let mut missing = core_file();
    missing.declarations.retain(
        |declaration| !matches!(declaration, Decl::Function(function) if function.name.text == NAME),
    );
    let errors = lower(&[missing, file(vec![fun("main", vec![])])])
        .expect_err("a missing initialization cycle thrower must reject the core");
    assert!(errors.iter().any(|error| {
        error.message
            == "scoop.core must define exactly one internal function `__scoopThrowInitializationCycle(message: String): Unit`"
    }));

    let mut public = core_file();
    let declaration = public
        .declarations
        .iter_mut()
        .find(|declaration| {
            matches!(declaration, Decl::Function(function) if function.name.text == NAME)
        })
        .expect("core test fixture declares the initialization cycle thrower");
    let Decl::Function(function) = declaration else {
        unreachable!("the selected declaration is a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    let errors = lower(&[public, file(vec![fun("main", vec![])])])
        .expect_err("a public initialization cycle thrower must reject the core");
    assert!(errors.iter().any(|error| {
        error.message
            == "function `__scoopThrowInitializationCycle` in scoop.core must be internal, non-generic, non-suspend, managed, and have signature `(String) -> Unit`"
    }));
}

#[test]
fn compiler_exception_core_rejects_missing_or_nonzero_arg_targets() {
    let mut missing = core_file();
    missing.declarations.retain(
        |declaration| !matches!(declaration, Decl::Class(class) if class.name.text == "UnwrapException"),
    );
    let errors = lower(&[missing, file(vec![fun("main", vec![])])])
        .expect_err("a missing compiler exception must reject the core");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "scoop.core must define class `UnwrapException`")
    );

    let mut malformed = core_file();
    let declaration = malformed
        .declarations
        .iter_mut()
        .find(|declaration| {
            matches!(declaration, Decl::Class(class) if class.name.text == "ArithmeticException")
        })
        .expect("ArithmeticException declaration");
    *declaration = class_decl(
        ast::ClassModifier::Final,
        "ArithmeticException",
        vec![(false, "code", ty_named("Int"))],
        Some(("Throwable", vec![])),
        vec![],
        vec![],
    );
    let errors = lower(&[malformed, file(vec![fun("main", vec![])])])
        .expect_err("a nonzero-argument compiler exception must reject the core");
    assert!(errors.iter().any(|error| {
        error.message
            == "class `ArithmeticException` in scoop.core must be a non-generic final subtype of `Throwable` callable with zero source arguments"
    }));
}

#[test]
fn compiler_exception_core_rejects_an_invalid_default_protocol_without_panicking() {
    let mut malformed = core_file();
    let declaration = malformed
        .declarations
        .iter_mut()
        .find(|declaration| {
            matches!(declaration, Decl::Class(class) if class.name.text == "IllegalStateException")
        })
        .expect("IllegalStateException declaration");
    let Decl::Class(class) = declaration else {
        unreachable!("the selected declaration is a class")
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut class.constructor else {
        unreachable!("IllegalStateException declares a primary constructor")
    };
    constructor.parameters[0].syntax = ast::ParameterSyntax::Default {
        expression: int_lit(1),
        equals_span: sp(),
    };

    let errors = lower(&[malformed, file(vec![fun("main", vec![])])])
        .expect_err("an ill-typed default cannot satisfy the compiler exception contract");
    assert!(errors.iter().any(|error| {
        error.message
            == "default value of parameter `message` in `IllegalStateException` must be of type Option<String>, found Int"
    }));
    assert!(errors.iter().any(|error| {
        error.message
            == "class `IllegalStateException` in scoop.core must be a non-generic final subtype of `Throwable` callable with zero source arguments"
    }));
}
