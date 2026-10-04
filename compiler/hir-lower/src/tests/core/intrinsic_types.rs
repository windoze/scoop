use super::super::*;

fn intrinsic_annotation(name: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(name.to_string()),
            span: sp(),
        }],
        span: sp(),
    }
}

fn marker_annotation(name: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident(name),
        args: Vec::new(),
        span: sp(),
    }
}

fn integer_intrinsic_method(
    name: &str,
    intrinsic: String,
    params: Vec<(&str, TypeRef)>,
    return_ty: TypeRef,
    operator: bool,
    infix: bool,
    no_gc: bool,
) -> ast::FunctionDecl {
    let mut method = method_full(
        false,
        false,
        name,
        params,
        Some(return_ty),
        FunctionBody::None,
    );
    if no_gc {
        method.annotations.push(marker_annotation("NoGC"));
    }
    method.annotations.push(intrinsic_annotation(&intrinsic));
    method.operator = operator.then_some(ast::OperatorModifier { span: sp() });
    method.infix = infix.then_some(ast::InfixModifier { span: sp() });
    method
}

fn integer_methods(kind: hir::IntegerKind) -> Vec<ast::FunctionDecl> {
    let type_name = kind.canonical_name();
    let prefix = kind.registry_key();
    let mut methods = Vec::new();
    for (name, key) in [
        ("unaryPlus", "unary_plus"),
        ("unaryMinus", "unary_minus"),
        ("inc", "inc"),
        ("dec", "dec"),
    ] {
        methods.push(integer_intrinsic_method(
            name,
            format!("{prefix}_{key}"),
            Vec::new(),
            ty_named(type_name),
            true,
            false,
            true,
        ));
    }
    for (name, key) in [("plus", "add"), ("minus", "sub"), ("times", "mul")] {
        methods.push(integer_intrinsic_method(
            name,
            format!("{prefix}_{key}"),
            vec![("other", ty_named(type_name))],
            ty_named(type_name),
            true,
            false,
            true,
        ));
    }
    for (name, key) in [("div", "div"), ("rem", "rem")] {
        methods.push(integer_intrinsic_method(
            name,
            format!("{prefix}_{key}"),
            vec![("other", ty_named(type_name))],
            ty_named(type_name),
            true,
            false,
            false,
        ));
    }
    methods.push(integer_intrinsic_method(
        "compareTo",
        format!("{prefix}_compare_to"),
        vec![("other", ty_named(type_name))],
        ty_named("Long"),
        true,
        false,
        true,
    ));
    methods.push(integer_intrinsic_method(
        "equals",
        format!("{prefix}_equals"),
        vec![("other", ty_named(type_name))],
        ty_named("Boolean"),
        true,
        false,
        true,
    ));
    for (name, key) in [("and", "and"), ("or", "or"), ("xor", "xor")] {
        methods.push(integer_intrinsic_method(
            name,
            format!("{prefix}_{key}"),
            vec![("other", ty_named(type_name))],
            ty_named(type_name),
            false,
            true,
            true,
        ));
    }
    methods.push(integer_intrinsic_method(
        "inv",
        format!("{prefix}_inv"),
        Vec::new(),
        ty_named(type_name),
        false,
        false,
        true,
    ));
    for (name, key) in [("shl", "shl"), ("shr", "shr"), ("ushr", "ushr")] {
        if name == "ushr" && kind.signedness() == hir::IntegerSignedness::Unsigned {
            continue;
        }
        methods.push(integer_intrinsic_method(
            name,
            format!("{prefix}_{key}"),
            vec![("count", ty_named("Long"))],
            ty_named(type_name),
            false,
            true,
            true,
        ));
    }
    for target in hir::IntegerKind::ALL {
        let source_name = match target {
            hir::IntegerKind::SIGNED_8 => "toInt8",
            hir::IntegerKind::SIGNED_16 => "toInt16",
            hir::IntegerKind::SIGNED_32 => "toInt32",
            hir::IntegerKind::SIGNED_64 => "toInt64",
            hir::IntegerKind::UNSIGNED_8 => "toUInt8",
            hir::IntegerKind::UNSIGNED_16 => "toUInt16",
            hir::IntegerKind::UNSIGNED_32 => "toUInt32",
            hir::IntegerKind::UNSIGNED_64 => "toUInt64",
        };
        methods.push(integer_intrinsic_method(
            source_name,
            format!("{prefix}_to_{}", target.registry_key()),
            Vec::new(),
            ty_named(target.canonical_name()),
            false,
            false,
            true,
        ));
    }
    let (conversion, to_string_helper, hash_helper) = match kind.signedness() {
        hir::IntegerSignedness::Signed => ("toInt64", "coreLongToString", "coreLongHash"),
        hir::IntegerSignedness::Unsigned => ("toUInt64", "coreULongToString", "coreULongHash"),
    };
    let carrier = || {
        if matches!(
            kind,
            hir::IntegerKind::SIGNED_64 | hir::IntegerKind::UNSIGNED_64
        ) {
            this_expr()
        } else {
            method_call(this_expr(), conversion, Vec::new())
        }
    };
    methods.extend([
        method_full(
            true,
            false,
            "toString",
            Vec::new(),
            Some(ty_named("String")),
            FunctionBody::Expr(Box::new(call(to_string_helper, vec![carrier()]))),
        ),
        method_full(
            true,
            false,
            "hash",
            Vec::new(),
            Some(ty_named("Long")),
            FunctionBody::Expr(Box::new(call(hash_helper, vec![carrier()]))),
        ),
    ]);
    methods
}

pub(super) fn intrinsic_type_declarations() -> Vec<Decl> {
    let annotation = intrinsic_annotation;
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
    let primitive_methods =
        |type_name: &str, equals_helper: &str, to_string_helper: &str, hash_helper: &str| {
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
                Some(ty_named("Long")),
                FunctionBody::Expr(Box::new(call(hash_helper, vec![this_expr()]))),
            );
            let mut methods = vec![equals_method];
            match type_name {
                "Boolean" => methods.push(intrinsic_operator(
                    "not",
                    "boolean_not",
                    vec![],
                    ty_named("Boolean"),
                )),
                _ => unreachable!("the primitive core helper only builds Boolean"),
            }
            methods.extend([to_string_method, hash_method]);
            methods
        };
    let strukt = |name: &str, intrinsic: &str, methods: Vec<ast::FunctionDecl>| {
        Decl::Struct(AstStructDecl {
            annotations: vec![annotation(intrinsic)],
            visibility: ast::VisibilitySyntax::Omitted,
            name: ident(name),
            type_params: Vec::new(),
            fields: ast::StructRepresentationDecl::Omitted,
            supertypes: ["ToString", "Hash"]
                .into_iter()
                .map(|name| ast::SupertypeSpec {
                    ty: ty_named(name),
                    constructor_arguments: None,
                    span: sp(),
                })
                .collect(),
            where_clause: None,
            members: methods
                .into_iter()
                .map(Box::new)
                .map(ast::StructMember::Function)
                .collect(),
            span: sp(),
        })
    };
    let class = |name: &str, intrinsic: &str, type_params: Vec<&str>| {
        Decl::Class(ast::ClassDecl {
            annotations: vec![annotation(intrinsic)],
            visibility: ast::VisibilitySyntax::Omitted,
            modifier: ast::ClassModifier::Final,
            name: ident(name),
            type_params: type_params.into_iter().map(type_param).collect(),
            constructor: ast::ClassConstructorDecl::Omitted,
            supertypes: Vec::new(),
            where_clause: None,
            members: Vec::new(),
            span: sp(),
        })
    };
    let mut string = class("String", "core_string", Vec::new());
    let Decl::Class(string_decl) = &mut string else {
        unreachable!()
    };
    string_decl.supertypes = ["ToString", "Hash"]
        .into_iter()
        .map(|name| ast::SupertypeSpec {
            ty: ty_named(name),
            constructor_arguments: None,
            span: sp(),
        })
        .collect();
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
        Some(ty_named("Long")),
        FunctionBody::Expr(Box::new(call("coreStringHash", vec![this_expr()]))),
    );
    string_decl.members = vec![
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
            ty_named("Long"),
        ),
        string_to_string,
        string_hash,
    ]
    .into_iter()
    .map(ast::ClassMember::Function)
    .collect();

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
    array_decl.members.extend(
        [
            intrinsic_operator(
                "get",
                "array_get",
                vec![("index", ty_named("Long"))],
                ty_named("T"),
            ),
            to_mutable,
        ]
        .into_iter()
        .map(ast::ClassMember::Function),
    );

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
    mutable_array_decl.members.extend(
        [
            intrinsic_operator(
                "get",
                "mutable_array_get",
                vec![("index", ty_named("Long"))],
                ty_named("T"),
            ),
            intrinsic_operator(
                "set",
                "mutable_array_set",
                vec![("index", ty_named("Long")), ("value", ty_named("T"))],
                ty_named("Unit"),
            ),
            to_immutable,
        ]
        .into_iter()
        .map(ast::ClassMember::Function),
    );

    super::arrays::add_length_getter(array_decl, "array_length");
    super::arrays::add_length_getter(mutable_array_decl, "mutable_array_length");

    let mut declarations = hir::IntegerKind::ALL
        .into_iter()
        .map(|kind| {
            strukt(
                kind.canonical_name(),
                kind.intrinsic_name(),
                integer_methods(kind),
            )
        })
        .collect::<Vec<_>>();
    declarations.extend([
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
    ]);
    declarations.extend([
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
            "coreLongToString",
            "scoop_rt_long_to_string",
            vec![("value", ty_named("Long"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreULongToString",
            "scoop_rt_ulong_to_string",
            vec![("value", ty_named("ULong"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreBooleanToString",
            "scoop_rt_bool_to_string",
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("String")),
        ),
        scoop_extern_fun(
            "coreLongHash",
            "scoop_rt_long_hash",
            vec![("value", ty_named("Long"))],
            Some(ty_named("Long")),
        ),
        scoop_extern_fun(
            "coreULongHash",
            "scoop_rt_ulong_hash",
            vec![("value", ty_named("ULong"))],
            Some(ty_named("Long")),
        ),
        scoop_extern_fun(
            "coreBooleanHash",
            "scoop_rt_bool_hash",
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Long")),
        ),
        scoop_extern_fun(
            "coreStringHash",
            "scoop_rt_string_hash",
            vec![("value", ty_named("String"))],
            Some(ty_named("Long")),
        ),
    ]);
    declarations
}
