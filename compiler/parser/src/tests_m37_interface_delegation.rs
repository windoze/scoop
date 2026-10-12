use crate::tests::{err, ok};
use scoop_ast::{Decl, Expr, SupertypeInitialization};

#[test]
fn delegates_retain_expressions_and_leave_the_class_body() {
    let file = ok(
        "class Host<T>(value: T) : Base(), I<T> by make(value), J by other where T : Bound { fun run() {} }",
    );
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("class")
    };
    assert_eq!(class.supertypes.len(), 3);
    assert!(class.where_clause.is_some());
    assert!(class.supertypes[0].constructor_arguments().is_some());
    let SupertypeInitialization::Delegate {
        expression,
        by_span,
    } = &class.supertypes[1].initialization
    else {
        panic!("delegate")
    };
    assert!(matches!(expression.as_ref(), Expr::Call(call) if call.args.len() == 1));
    assert_eq!(by_span.end - by_span.start, 2);
    assert_eq!(class.functions().count(), 1);
    let file = ok("object Host : I by make() { fun run() {} }");
    let Decl::Object(object) = &file.declarations[0] else {
        panic!("object")
    };
    assert_eq!(object.members.len(), 1);
}

#[test]
fn grouped_delegate_expressions_allow_trailing_lambdas() {
    let file = ok("class Host : I by (make { 1 }) { fun run() {} }");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("class")
    };
    let SupertypeInitialization::Delegate { expression, .. } = &class.supertypes[0].initialization
    else {
        panic!("delegate")
    };
    assert!(matches!(expression.as_ref(), Expr::Call(call) if call.args.len() == 1));
    assert_eq!(class.functions().count(), 1);
}

#[test]
fn enums_reject_delegate_storage_and_by_requires_an_expression() {
    assert!(
        err("enum Flag : I by value { On }")
            .1
            .contains("only allowed in class and object")
    );
    assert!(!err("class Host : I by").1.is_empty());
}
