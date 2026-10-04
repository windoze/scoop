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
    let defaulted_exception = |name: &str, message: &str| {
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
        parameter.syntax = ast::ParameterSyntax::Default {
            expression: some(str_lit(message)),
            equals_span: sp(),
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
        subclass("IndexOutOfBoundsException", "array index out of bounds"),
        defaulted_exception("IllegalArgumentException", "illegal argument"),
        defaulted_exception("IllegalStateException", "illegal state"),
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
