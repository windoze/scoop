use super::*;

#[test]
fn class_construction_lowers_to_class_init() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int")), (false, "a", ty_named("Any"))],
            None,
            vec![],
            vec![],
        ),
        fun(
            "main",
            vec![val("c", call("C", vec![int_lit(1), int_lit(2)]))],
        ),
    ]);
    let module = lower_user(file).expect("class construction must lower");
    let main = body_of(&module, "main");
    match &local_init(main, "c").kind {
        hir::ExprKind::ClassInit {
            constructor, args, ..
        } => {
            let application = module.class_constructor_applications[*constructor].owner;
            let class_id = module.class_applications[application].template;
            assert_eq!(module.classes[class_id].name, "C");
            assert!(matches!(
                module.types[local_init(main, "c").ty],
                hir::Type::Class(found) if found == application
            ));
            assert_eq!(args.len(), 2);
            assert!(
                args.iter()
                    .all(|argument| matches!(argument.kind, hir::ExprKind::Local(_)))
            );
            // The Int argument crossing into the `Any` property boxes.
            assert!(main.statements.iter().any(|statement| matches!(
                &statement.kind,
                hir::StatementKind::ValDecl {
                    init: hir::Expr {
                        kind: hir::ExprKind::Box(_),
                        ..
                    },
                    ..
                }
            )));
        }
        other => panic!("expected a ClassInit, found {other:?}"),
    }
}

#[test]
fn class_construction_arity_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `C` in nominal constructor candidate layer:\n  - class C(x: Int) — expects 1 argument(s), but 0 were supplied"
    );
}

#[test]
fn class_construction_argument_types_are_checked() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![str_lit("s")]))]),
    ]);
    let errors = lower_user(file).expect_err("a wrong argument type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `C` in nominal constructor candidate layer:\n  - class C(x: Int) — argument for `x` has type String, which is not a subtype of Int"
    );
}
