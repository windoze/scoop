use super::super::*;

fn make_vararg(parameter: &mut ast::Param) {
    parameter.syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
}

fn c_extern(name: &str, params: Vec<(&str, ast::TypeRef)>) -> Decl {
    let mut declaration = fun_sig(name, vec![], params, None, vec![]);
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.annotations = vec![ast::Annotation {
        name: ident("Extern"),
        args: [("lib", ""), ("name", "native_values"), ("abi", "c")]
            .into_iter()
            .map(|(name, value)| ast::AnnotationArg {
                name: Some(ident(name)),
                value: ast::AnnotationLiteral::String(value.to_string()),
                span: sp(),
            })
            .collect(),
        span: sp(),
    }];
    function.body = ast::FunctionBody::None;
    declaration
}

#[test]
fn vararg_declarations_use_exact_array_runtime_types() {
    let mut function = fun_sig(
        "collect",
        vec![],
        vec![("values", ty_named("Int"))],
        None,
        vec![],
    );
    let Decl::Function(function_decl) = &mut function else {
        unreachable!()
    };
    make_vararg(&mut function_decl.params[0]);

    let mut strukt = struct_decl("Numbers", vec![("values", ty_named("Int"))]);
    let Decl::Struct(struct_decl) = &mut strukt else {
        unreachable!()
    };
    let ast::StructRepresentationDecl::Declared(fields) = &mut struct_decl.fields else {
        unreachable!()
    };
    fields[0].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };

    let mut class = class_decl(
        ast::ClassModifier::Final,
        "BoxedNumbers",
        vec![(false, "values", ty_named("Int"))],
        None,
        vec![],
        vec![],
    );
    let Decl::Class(class_decl) = &mut class else {
        unreachable!()
    };
    let ast::ClassConstructorDecl::Declared(properties) = &mut class_decl.constructor else {
        unreachable!()
    };
    properties[0].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };

    let mut enumeration = enum_decl(
        "NumberList",
        vec![],
        vec![variant_constructor(
            "Values",
            vec![("values", ty_named("Int"), None)],
        )],
    );
    let Decl::Enum(enum_decl) = &mut enumeration else {
        unreachable!()
    };
    let ast::VariantDeclKind::Constructor(fields) = &mut enum_decl.variants[0].kind else {
        unreachable!()
    };
    fields[0].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };

    let module = lower_user(file(vec![
        function,
        strukt,
        class,
        enumeration,
        fun("main", vec![]),
    ]))
    .expect("all source parameter kinds use Array<Int> as their runtime type");

    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "collect")
        .expect("collect function");
    assert_eq!(hir::type_name(&module, function.params[0].ty), "Array<Int>");

    let (_, strukt) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Numbers")
        .expect("Numbers struct");
    assert_eq!(
        hir::type_name(&module, strukt.semantic_fields()[0].ty),
        "Array<Int>"
    );

    let (_, class) = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "BoxedNumbers")
        .expect("BoxedNumbers class");
    assert_eq!(
        hir::type_name(&module, class.semantic_constructor()[0].ty),
        "Array<Int>"
    );

    let (_, enumeration) = module
        .enums
        .iter()
        .find(|(_, declaration)| declaration.name == "NumberList")
        .expect("NumberList enum");
    assert_eq!(
        hir::type_name(&module, enumeration.variants[0].fields[0].ty),
        "Array<Int>"
    );
}

#[test]
fn c_abi_extern_rejects_language_vararg() {
    let mut declaration = c_extern("nativeValues", vec![("values", ty_named("Int"))]);
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    make_vararg(&mut function.params[0]);

    let errors = lower_user(file(vec![declaration, fun("main", vec![])]))
        .expect_err("C ABI language vararg must be rejected")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert!(errors.iter().any(|message| {
        message
            == "C ABI extern function `nativeValues` cannot declare a language `vararg` parameter"
    }));
}
