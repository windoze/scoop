use super::*;

#[test]
fn unimplemented_interface_method_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(Final, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `C` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn interface_implementation_with_the_wrong_signature_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec!["Describable"],
            vec![method_full(
                true,
                false,
                "describe",
                vec![],
                Some(ty_named("Int")),
                FunctionBody::Expr(Box::new(int_lit(1))),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a signature mismatch must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message
                == "class `C` does not implement interface method `Describable.describe`"),
        "unexpected diagnostics: {errors:?}"
    );
    assert!(
        errors.iter().any(
            |e| e.message == "`describe` is marked `override` but does not override any method"
        ),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn abstract_classes_may_leave_interface_methods_unimplemented() {
    let file = file(vec![
        describable(),
        class_decl(Abstract, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    lower_user(file).expect("abstract classes defer the implementation");
}

// --- negative: abstract and bodies ---

#[test]
fn abstract_class_instantiation_is_an_error() {
    let file = file(vec![
        class_decl(
            Abstract,
            "Base",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "kind", vec![], Some(ty_named("Int")))],
        ),
        fun("main", vec![stmt(call("Base", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("instantiating an abstract class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract class `Base` cannot be instantiated"
    );
}

#[test]
fn abstract_method_outside_an_abstract_class_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("misplaced abstract must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract function `m` is only allowed in abstract classes"
    );
}

#[test]
fn interface_method_with_a_body_is_a_default() {
    let file = file(vec![
        interface_decl("I", vec![method("m", vec![], None, vec![])]),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("a bodied interface method is a default");
    let (_, interface) = module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "I")
        .expect("I interface");
    assert_eq!(interface.methods.len(), 1);
    assert_eq!(
        module.interface_methods[interface.methods[0]].implementation,
        hir::InterfaceMemberImplementation::Body
    );
}

#[test]
fn concrete_method_without_a_body_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(false, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a bodyless concrete method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "function `m` must have a body");
}
