use super::*;

// --- qualified enum variant construction in method-call shape ---

/// The M6 parser folds `E.V(args)` into `MethodCall { receiver:
/// Var("E"), ... }`; when `E` is no variable but an enum, this is a
/// qualified variant construction, not a method call.
#[test]
fn qualified_variant_construction_in_method_call_shape() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![
                variant_positional("Circle", vec![ty_named("Int")]),
                variant_constructor("Named", vec![("w", ty_named("Int"), Some(int_lit(7)))]),
            ],
        ),
        fun(
            "main",
            vec![
                val("s", method_call(var("Shape"), "Circle", vec![int_lit(5)])),
                // Constructor-style defaults fill the tail.
                val("n", method_call(var("Shape"), "Named", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("qualified variants must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                assert_eq!(*variant, 0);
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(5)));
            }
            other => panic!("expected a variant construction, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
    let named = main
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl { init, .. } => match &init.kind {
                hir::ExprKind::VariantConstruct {
                    variant: 1, args, ..
                } => Some(args),
                _ => None,
            },
            _ => None,
        })
        .next()
        .expect("defaulted Named construction");
    assert_eq!(named.len(), 1);
    assert!(matches!(named[0].kind, hir::ExprKind::Local(_)));
}

#[test]
fn qualified_generic_variant_infers_type_arguments() {
    let file = file(vec![
        enum_decl(
            "Box",
            vec!["T"],
            vec![variant_positional("Wrap", vec![ty_named("T")])],
        ),
        fun(
            "main",
            vec![val_ty(
                "b",
                Some(ty_generic("Box", vec![ty_named("Int")])),
                method_call(var("Box"), "Wrap", vec![int_lit(1)]),
            )],
        ),
    ]);
    let module = lower_user(file).expect("generic variant construction must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::VariantConstruct { application, .. } => {
                let arguments = &module.enum_applications[*application].arguments;
                assert_eq!(arguments.len(), 1);
                assert_eq!(module.types[arguments[0]], hir::Type::Int);
            }
            other => panic!("expected a variant construction, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
}

#[test]
fn a_variable_shadows_the_enum_name() {
    // A local variable named `Shape` wins over the enum: the receiver
    // is a genuine method call on the variable (and fails as one).
    let file = file(vec![
        struct_s(),
        enum_decl(
            "Shape",
            vec![],
            vec![variant_positional("Circle", vec![ty_named("Int")])],
        ),
        fun(
            "main",
            vec![
                val("Shape", struct_init("S", vec![int_lit(1)])),
                val("s", method_call(var("Shape"), "Circle", vec![int_lit(5)])),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("the variable must shadow the enum");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `S` has no method `Circle`");
}

#[test]
fn unknown_qualified_variant_in_method_call_shape_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_positional("Circle", vec![ty_named("Int")])],
        ),
        fun(
            "main",
            vec![val(
                "s",
                method_call(var("Shape"), "Square", vec![int_lit(5)]),
            )],
        ),
    ]);
    let errors = lower_user(file).expect_err("an unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Shape` has no variant `Square`");
}
