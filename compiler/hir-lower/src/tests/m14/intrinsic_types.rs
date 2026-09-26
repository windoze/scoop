use super::*;

#[test]
fn intrinsic_type_contract_is_complete_in_export_and_local_hir() {
    let callback = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let mut native = fun_expr(
        "native",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let Decl::Function(native_function) = &mut native else {
        unreachable!("fun_expr builds a function declaration")
    };
    native_function.annotations.push(ast::Annotation {
        name: ident("NoGC"),
        args: Vec::new(),
        span: sp(),
    });
    let reference = Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("native"),
        span: sp(),
    };
    let address = Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude: 1,
        radix: ast::IntegerRadix::Decimal,
        suffix: ast::IntegerSuffix::UnsignedLong,
        span: sp(),
    });
    let output = lower_user_output(file(vec![
        fun_sig(
            "takePtr",
            Vec::new(),
            vec![("pointer", ty_generic("Ptr", vec![ty_named("Int")]))],
            None,
            Vec::new(),
        ),
        fun_sig(
            "takeFunPtr",
            Vec::new(),
            vec![("callback", ty_generic("FunPtr", vec![callback]))],
            None,
            Vec::new(),
        ),
        native,
        fun(
            "main",
            vec![
                Statement {
                    kind: StatementKind::SafetyBlock {
                        mode: ast::SafetyMode::Unsafe,
                        block: block(vec![stmt(call(
                            "takePtr",
                            vec![typed_call("Ptr", vec![ty_named("Int")], vec![address])],
                        ))]),
                    },
                    span: sp(),
                },
                stmt(call("takeFunPtr", vec![reference])),
            ],
        ),
    ]))
    .expect("the core intrinsic type contract must lower");
    let export = &output.export;
    let core = defined_export_core(export.module()).fundamental_types;
    for (kind, id) in core.integers.iter() {
        let hir::StructRepresentation::Intrinsic(declaration) = export.structs[id].representation
        else {
            panic!("fixed intrinsic struct must not masquerade as an empty declaration")
        };
        assert_eq!(declaration, hir::IntrinsicTypeKind::Integer(kind));
        assert_eq!(
            export.struct_applications[export.structs[id].self_application].representation,
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Integer(kind)
            )
        );
    }
    let hir::StructRepresentation::Intrinsic(boolean_declaration) =
        export.structs[core.boolean].representation
    else {
        panic!("Boolean must have an explicit intrinsic representation")
    };
    assert_eq!(boolean_declaration, hir::IntrinsicTypeKind::Boolean);

    let hir::ClassRepresentation::Intrinsic(string_declaration) =
        export.classes[core.string].representation
    else {
        panic!("String must have an explicit intrinsic representation")
    };
    assert_eq!(string_declaration, hir::IntrinsicTypeKind::String);
    assert!(matches!(
        export.class_applications[export.classes[core.string].self_application].representation,
        hir::ClassApplicationRepresentation::Intrinsic(hir::IntrinsicTypeRepresentation::String)
    ));
    for (id, mutable) in [(core.array, false), (core.mutable_array, true)] {
        let application = &export.class_applications[export.classes[id].self_application];
        let element = match &application.representation {
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Array { element },
            ) if !mutable => element,
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::MutableArray { element },
            ) if mutable => element,
            _ => panic!("generic intrinsic family must carry its element type"),
        };
        assert!(matches!(export.types[*element], hir::Type::Param(_)));
    }
    for (id, kind) in [
        (core.ptr, hir::IntrinsicTypeKind::Ptr),
        (core.fun_ptr, hir::IntrinsicTypeKind::FunPtr),
    ] {
        let hir::StructRepresentation::Intrinsic(declaration) = export.structs[id].representation
        else {
            panic!("pointer family must have an intrinsic representation")
        };
        assert_eq!(declaration, kind);
        assert!(export.structs[id].semantic_fields().is_empty());
        let application = &export.struct_applications[export.structs[id].self_application];
        match (&application.representation, kind) {
            (
                hir::StructApplicationRepresentation::Intrinsic(
                    hir::IntrinsicTypeRepresentation::Ptr { pointee },
                ),
                hir::IntrinsicTypeKind::Ptr,
            ) => assert!(matches!(export.types[*pointee], hir::Type::Param(_))),
            (
                hir::StructApplicationRepresentation::Intrinsic(
                    hir::IntrinsicTypeRepresentation::FunPtr { function },
                ),
                hir::IntrinsicTypeKind::FunPtr,
            ) => assert!(matches!(export.types[*function], hir::Type::Param(_))),
            _ => panic!("pointer family self application must preserve its deferred argument"),
        }
    }
    assert_eq!(
        export.structs[core.ptr].gc_free_pointee_requirements.len(),
        1
    );
    assert_eq!(
        export.structs[core.ptr].gc_free_pointee_requirements[0].type_param,
        export.structs[core.ptr].type_params[0].id
    );
    assert!(
        export.structs[core.fun_ptr]
            .gc_free_pointee_requirements
            .is_empty()
    );

    let local = &output.local;
    let fundamental_types = &defined_concrete_core(local).fundamental_types;
    for (kind, id) in fundamental_types.integers.iter() {
        assert!(matches!(
            &local.structs[id].representation,
            hir::concrete::StructRepresentation::Intrinsic { application, .. }
                if application == &hir::concrete::IntrinsicTypeRepresentation::Integer(kind)
        ));
    }
    assert!(matches!(
        &local.structs[fundamental_types.boolean].representation,
        hir::concrete::StructRepresentation::Intrinsic {
            application: hir::concrete::IntrinsicTypeRepresentation::Boolean,
            ..
        }
    ));
    assert!(matches!(
        local.classes[fundamental_types.string].representation,
        hir::concrete::ClassRepresentation::Intrinsic {
            application: hir::concrete::IntrinsicTypeRepresentation::String,
            ..
        }
    ));
    let concrete_int = concrete_int_type(local);
    assert!(local.types.iter().any(|(_, ty)| {
        matches!(ty.kind, hir::concrete::TypeKind::Ptr(pointee) if pointee == concrete_int)
    }));
    assert!(
        local
            .types
            .iter()
            .any(|(_, ty)| matches!(ty.kind, hir::concrete::TypeKind::FunPtr(_)))
    );
}

#[test]
fn intrinsic_type_shape_is_validated_at_its_source() {
    let mut core = core_file();
    let int = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(declaration) if declaration.name.text == "Int" => Some(declaration),
            _ => None,
        })
        .expect("core Int declaration");
    int.fields = ast::StructRepresentationDecl::Declared(Vec::new());
    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("an intrinsic type must omit its compiler representation");
    assert!(errors.iter().any(|error| {
        error.file == 0
            && error.message
                == "intrinsic type `core_int` must omit its fields or primary constructor"
    }));
}

#[test]
fn intrinsic_types_do_not_acquire_zero_argument_source_constructors() {
    let errors = lower_user_output(file(vec![fun(
        "main",
        vec![stmt(call("Int", vec![])), stmt(call("String", vec![]))],
    )]))
    .expect_err("hidden intrinsic construction is not a source constructor");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic struct `Int` has no source constructor")
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic class `String` has no source constructor")
    );
}
