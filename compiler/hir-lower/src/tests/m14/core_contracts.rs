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
    let output = lower_user_output(file(vec![fun("main", vec![])]))
        .expect("the canonical compiler exception core must lower");

    let export = output.export.exception_core;
    let export_names = [
        export.throwable,
        export.unwrap_exception,
        export.class_cast_exception,
        export.arithmetic_exception,
        export.index_out_of_bounds_exception,
        export.illegal_state_exception,
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
        ]
    );

    let local = output.local.exception_core;
    let local_exceptions = [
        local.throwable,
        local.unwrap_exception,
        local.class_cast_exception,
        local.arithmetic_exception,
        local.index_out_of_bounds_exception,
        local.illegal_state_exception,
    ];
    let local_names =
        local_exceptions.map(|exception| output.local.classes[exception.class()].name.as_str());
    assert_eq!(local_names, export_names);
    for exception in local_exceptions {
        let constructor = &output.local.class_constructors[exception.callable()];
        assert_eq!(constructor.class, exception.class());
        assert!(constructor.params.is_empty());
        assert!(matches!(
            output.local.types[constructor.return_type].kind,
            hir::concrete::TypeKind::Class(class) if class == exception.class()
        ));
    }
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
            == "class `ArithmeticException` in scoop.core must be a non-generic final subtype of `Throwable` with a zero-argument constructor"
    }));
}
