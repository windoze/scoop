use super::super::*;

pub(super) fn intrinsic_type_declarations() -> Vec<Decl> {
    let annotation = |name: &str| ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(name.to_string()),
            span: sp(),
        }],
        span: sp(),
    };
    let intrinsic_operator =
        |name: &str, intrinsic: &str, params: Vec<(&str, TypeRef)>, return_ty: TypeRef| {
            let mut method = method_full(
                false,
                false,
                name,
                params,
                Some(return_ty),
                FunctionBody::None,
            );
            method.annotations.push(annotation(intrinsic));
            method.operator = Some(ast::OperatorModifier { span: sp() });
            method
        };
    let primitive_methods = |type_name: &str,
                             equals_helper: &str,
                             to_string_helper: &str,
                             hash_helper: &str| {
        let mut equals_method = method_full(
            false,
            false,
            "equals",
            vec![("other", ty_named(type_name))],
            Some(ty_named("Boolean")),
            FunctionBody::Expr(Box::new(call(
                equals_helper,
                vec![this_expr(), var("other")],
            ))),
        );
        equals_method.operator = Some(ast::OperatorModifier { span: sp() });
        let to_string_method = method_full(
            true,
            false,
            "toString",
            Vec::new(),
            Some(ty_named("String")),
            FunctionBody::Expr(Box::new(call(to_string_helper, vec![this_expr()]))),
        );
        let hash_method = method_full(
            true,
            false,
            "hash",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::Expr(Box::new(call(hash_helper, vec![this_expr()]))),
        );
        let mut methods = vec![equals_method];
        match type_name {
            "Int" => {
                methods.extend([
                    intrinsic_operator("unaryPlus", "int_unary_plus", vec![], ty_named("Int")),
                    intrinsic_operator("unaryMinus", "int_unary_minus", vec![], ty_named("Int")),
                    intrinsic_operator("inc", "int_inc", vec![], ty_named("Int")),
                    intrinsic_operator("dec", "int_dec", vec![], ty_named("Int")),
                ]);
                for (name, intrinsic) in [
                    ("plus", "int_add"),
                    ("minus", "int_sub"),
                    ("times", "int_mul"),
                    ("div", "int_div"),
                    ("rem", "int_rem"),
                    ("compareTo", "int_compare_to"),
                ] {
                    methods.push(intrinsic_operator(
                        name,
                        intrinsic,
                        vec![("other", ty_named("Int"))],
                        ty_named("Int"),
                    ));
                }
            }
            "UInt" => {
                methods.extend([
                    intrinsic_operator("unaryPlus", "uint_unary_plus", vec![], ty_named("UInt")),
                    intrinsic_operator("inc", "uint_inc", vec![], ty_named("UInt")),
                    intrinsic_operator("dec", "uint_dec", vec![], ty_named("UInt")),
                ]);
                for (name, intrinsic) in [
                    ("plus", "uint_add"),
                    ("minus", "uint_sub"),
                    ("times", "uint_mul"),
                    ("div", "uint_div"),
                    ("rem", "uint_rem"),
                ] {
                    methods.push(intrinsic_operator(
                        name,
                        intrinsic,
                        vec![("other", ty_named("UInt"))],
                        ty_named("UInt"),
                    ));
                }
                methods.push(intrinsic_operator(
                    "compareTo",
                    "uint_compare_to",
                    vec![("other", ty_named("UInt"))],
                    ty_named("Int"),
                ));
            }
            "Boolean" => methods.push(intrinsic_operator(
                "not",
                "boolean_not",
                vec![],
                ty_named("Boolean"),
            )),
            _ => unreachable!("the primitive core helper has a closed type set"),
        }
        methods.extend([to_string_method, hash_method]);
        methods
    };
    let strukt = |name: &str, intrinsic: &str, methods: Vec<ast::FunctionDecl>| {
        Decl::Struct(AstStructDecl {
            annotations: vec![annotation(intrinsic)],
            name: ident(name),
            type_params: Vec::new(),
            fields: ast::StructRepresentationDecl::Omitted,
            interfaces: vec![ty_named("ToString"), ty_named("Hash")],
            where_clause: None,
            methods,
            span: sp(),
        })
    };
    let class = |name: &str, intrinsic: &str, type_params: Vec<&str>| {
        Decl::Class(ast::ClassDecl {
            annotations: vec![annotation(intrinsic)],
            modifier: ast::ClassModifier::Final,
            name: ident(name),
            type_params: type_params.into_iter().map(type_param).collect(),
            constructor: ast::ClassConstructorDecl::Omitted,
            base_class: None,
            interfaces: Vec::new(),
            where_clause: None,
            methods: Vec::new(),
            span: sp(),
        })
    };
    let mut string = class("String", "core_string", Vec::new());
    let Decl::Class(string_decl) = &mut string else {
        unreachable!()
    };
    string_decl.interfaces = vec![ty_named("ToString"), ty_named("Hash")];
    let mut string_equals = method_full(
        false,
        false,
        "equals",
        vec![("other", ty_named("String"))],
        Some(ty_named("Boolean")),
        FunctionBody::Expr(Box::new(call(
            "coreStringEquals",
            vec![this_expr(), var("other")],
        ))),
    );
    string_equals.operator = Some(ast::OperatorModifier { span: sp() });
    let string_to_string = method_full(
        true,
        false,
        "toString",
        Vec::new(),
        Some(ty_named("String")),
        FunctionBody::Expr(Box::new(this_expr())),
    );
    let string_hash = method_full(
        true,
        false,
        "hash",
        Vec::new(),
        Some(ty_named("Int")),
        FunctionBody::Expr(Box::new(call("coreStringHash", vec![this_expr()]))),
    );
    string_decl.methods = vec![
        string_equals,
        intrinsic_operator(
            "plus",
            "string_concat",
            vec![("other", ty_named("String"))],
            ty_named("String"),
        ),
        intrinsic_operator(
            "compareTo",
            "string_compare_to",
            vec![("other", ty_named("String"))],
            ty_named("Int"),
        ),
        string_to_string,
        string_hash,
    ];

    let mut array = class("Array", "core_array", vec!["T"]);
    let Decl::Class(array_decl) = &mut array else {
        unreachable!()
    };
    let mut to_mutable = method_full(
        false,
        false,
        "toMutableArray",
        Vec::new(),
        Some(ty_generic("MutableArray", vec![ty_named("T")])),
        FunctionBody::None,
    );
    to_mutable.annotations.push(annotation("array_to_mutable"));
    array_decl.methods.extend([
        intrinsic_operator(
            "get",
            "array_get",
            vec![("index", ty_named("Int"))],
            ty_named("T"),
        ),
        to_mutable,
    ]);

    let mut mutable_array = class("MutableArray", "core_mutable_array", vec!["T"]);
    let Decl::Class(mutable_array_decl) = &mut mutable_array else {
        unreachable!()
    };
    let mut to_immutable = method_full(
        false,
        false,
        "toArray",
        Vec::new(),
        Some(ty_generic("Array", vec![ty_named("T")])),
        FunctionBody::None,
    );
    to_immutable
        .annotations
        .push(annotation("array_to_immutable"));
    mutable_array_decl.methods.extend([
        intrinsic_operator(
            "get",
            "mutable_array_get",
            vec![("index", ty_named("Int"))],
            ty_named("T"),
        ),
        intrinsic_operator(
            "set",
            "mutable_array_set",
            vec![("index", ty_named("Int")), ("value", ty_named("T"))],
            ty_named("Unit"),
        ),
        to_immutable,
    ]);

    let mut declarations = vec![
        strukt(
            "Int",
            "core_int",
            primitive_methods("Int", "coreIntEquals", "coreIntToString", "coreIntHash"),
        ),
        strukt(
            "UInt",
            "core_uint",
            primitive_methods("UInt", "coreUIntEquals", "coreUIntToString", "coreUIntHash"),
        ),
        strukt(
            "Boolean",
            "core_boolean",
            primitive_methods(
                "Boolean",
                "coreBooleanEquals",
                "coreBooleanToString",
                "coreBooleanHash",
            ),
        ),
        string,
        array,
        mutable_array,
    ];
    declarations.extend([
        scoop_extern_fun(
            "coreIntEquals",
            "scoop_rt_int_equals",
            vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
            Some(ty_named("Boolean")),
        ),
        scoop_extern_fun(
            "coreUIntEquals",
            "scoop_rt_uint_equals",
            vec![("left", ty_named("UInt")), ("right", ty_named("UInt"))],
            Some(ty_named("Boolean")),
        ),
        scoop_extern_fun(
            "coreBooleanEquals",
            "scoop_rt_bool_equals",
            vec![
                ("left", ty_named("Boolean")),
                ("right", ty_named("Boolean")),
            ],
            Some(ty_named("Boolean")),
        ),
        scoop_extern_fun(
            "coreStringEquals",
            "scoop_rt_string_eq",
            vec![("left", ty_named("String")), ("right", ty_named("String"))],
            Some(ty_named("Boolean")),
        ),
        scoop_extern_fun(
            "coreIntToString",
            "scoop_rt_int_to_string",
            vec![("value", ty_named("Int"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreUIntToString",
            "scoop_rt_uint_to_string",
            vec![("value", ty_named("UInt"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreBooleanToString",
            "scoop_rt_bool_to_string",
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreIntHash",
            "scoop_rt_int_hash",
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
        ),
        scoop_extern_fun(
            "coreUIntHash",
            "scoop_rt_uint_hash",
            vec![("value", ty_named("UInt"))],
            Some(ty_named("Int")),
        ),
        scoop_extern_fun(
            "coreBooleanHash",
            "scoop_rt_bool_hash",
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Int")),
        ),
        scoop_extern_fun(
            "coreStringHash",
            "scoop_rt_string_hash",
            vec![("value", ty_named("String"))],
            Some(ty_named("Int")),
        ),
    ]);
    declarations
}
