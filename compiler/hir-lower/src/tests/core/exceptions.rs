use super::super::*;

pub(super) fn exception_core_declarations() -> Vec<Decl> {
    let subclass = |name: &str, message: &str| {
        class_decl(
            ast::ClassModifier::Final,
            name,
            vec![],
            Some(("Exception", vec![some(str_lit(message))])),
            vec![],
            vec![],
        )
    };
    let message_exception = |name: &str, message: Option<&str>| {
        let mut declaration = class_decl(
            ast::ClassModifier::Final,
            name,
            vec![(false, "message", ty_nullable(ty_named("String")))],
            Some(("Exception", vec![var("message")])),
            vec![],
            vec![],
        );
        let Decl::Class(class) = &mut declaration else {
            unreachable!("class declaration builder returns a class")
        };
        let ast::ClassConstructorDecl::Declared(constructor) = &mut class.constructor else {
            unreachable!("class declaration builder declares a primary constructor")
        };
        let parameter = &mut constructor.parameters[0];
        parameter.property = ast::PrimaryParameterProperty::Plain;
        parameter.member_visibility = None;
        parameter.syntax = match message {
            Some(message) => ast::ParameterSyntax::Default {
                expression: some(str_lit(message)),
                equals_span: sp(),
            },
            None => ast::ParameterSyntax::Required,
        };
        declaration
    };
    vec![
        class_decl(
            ast::ClassModifier::Open,
            "Throwable",
            vec![],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            ast::ClassModifier::Open,
            "Exception",
            vec![(false, "message", ty_nullable(ty_named("String")))],
            Some(("Throwable", vec![])),
            vec![],
            vec![],
        ),
        subclass("UnwrapException", "unwrap on None"),
        subclass("ClassCastException", "invalid cast"),
        subclass("ArithmeticException", "arithmetic error"),
        message_exception("MissingContextException", None),
        subclass("IndexOutOfBoundsException", "array index out of bounds"),
        message_exception("IllegalArgumentException", Some("illegal argument")),
        message_exception("IllegalStateException", Some("illegal state")),
        fun_sig(
            "__scoopThrowInitializationCycle",
            Vec::new(),
            vec![("message", ty_named("String"))],
            None,
            vec![throw_stmt(call(
                "IllegalStateException",
                vec![some(var("message"))],
            ))],
        ),
    ]
}
