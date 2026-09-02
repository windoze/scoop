use super::*;

/// The minimal `scoop.core` (sysroot): the `Option<T>` enum (spec 7.2),
/// the complete compiler exception core (spec 11.7), the M10 coroutine
/// protocol, plus the M7
/// `io.scoop` final shape
/// (docs/milestone7/DESIGN.md section 2) — the managed `write` extern
/// intrinsic and `print` / `println` as ordinary `Any`-parameter
/// functions dispatching `toString()`.
pub(crate) fn core_file() -> SourceFile {
    let mut declarations = capability_interfaces();
    declarations.extend(intrinsic_type_declarations());
    declarations.push(enum_decl(
        "Option",
        vec!["T"],
        vec![
            variant_positional("Some", vec![ty_named("T")]),
            variant_unit("None"),
        ],
    ));
    declarations.extend(exception_core_declarations());
    declarations.extend(coroutine_core_declarations());
    declarations.extend(ffi_core_declarations());
    let mut print = fun_expr(
        "print",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        call("write", vec![method_call(var("value"), "toString", vec![])]),
    );
    let Decl::Function(print_decl) = &mut print else {
        unreachable!()
    };
    print_decl.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ToString")));
    let mut println = fun_sig(
        "println",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![
            stmt(call(
                "write",
                vec![method_call(var("value"), "toString", vec![])],
            )),
            stmt(call("write", vec![str_lit("\n")])),
        ],
    );
    let Decl::Function(println_decl) = &mut println else {
        unreachable!()
    };
    println_decl.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ToString")));
    declarations.extend([
        scoop_extern_fun(
            "write",
            "scoop_rt_write",
            vec![("message", ty_named("String"))],
            None,
        ),
        print,
        println,
    ]);
    file(declarations)
}

fn capability_interfaces() -> Vec<Decl> {
    vec![
        interface_decl(
            "ToString",
            vec![method_full(
                false,
                true,
                "toString",
                Vec::new(),
                Some(ty_named("String")),
                FunctionBody::None,
            )],
        ),
        interface_decl(
            "Hash",
            vec![method_full(
                false,
                true,
                "hash",
                Vec::new(),
                Some(ty_named("Int")),
                FunctionBody::None,
            )],
        ),
    ]
}

fn intrinsic_type_declarations() -> Vec<Decl> {
    let annotation = |name: &str| ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(name.to_string()),
            span: sp(),
        }],
        span: sp(),
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
                Some(ty_named("Int")),
                FunctionBody::Expr(Box::new(call(hash_helper, vec![this_expr()]))),
            );
            vec![equals_method, to_string_method, hash_method]
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
    string_decl.methods = vec![string_equals, string_to_string, string_hash];

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
        class("Array", "core_array", vec!["T"]),
        class("MutableArray", "core_mutable_array", vec!["T"]),
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

fn ffi_core_declarations() -> Vec<Decl> {
    let marker = |name: &str| ast::Annotation {
        name: ident(name),
        args: Vec::new(),
        span: sp(),
    };
    let intrinsic = |name: &str| ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(name.to_string()),
            span: sp(),
        }],
        span: sp(),
    };
    let pointer_method = |name: &str,
                          intrinsic_name: &str,
                          type_params: Vec<&str>,
                          params: Vec<(&str, TypeRef)>,
                          return_ty: Option<TypeRef>| {
        let mut method = method_full(false, false, name, params, return_ty, FunctionBody::None);
        method.annotations = vec![marker("NoGC"), marker("Unsafe"), intrinsic(intrinsic_name)];
        method.type_params = type_params.into_iter().map(type_param).collect();
        for param in &mut method.type_params {
            param.inline_bound = Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));
        }
        method
    };

    let mut ptr = generic_struct_decl_full(
        "Ptr",
        vec!["T"],
        vec![("_rawPointer", ty_named("UInt"))],
        Vec::new(),
        vec![
            pointer_method(
                "toUInt",
                "ptr_to_uint",
                vec![],
                vec![],
                Some(ty_named("UInt")),
            ),
            pointer_method(
                "cast",
                "ptr_cast",
                vec!["U"],
                vec![],
                Some(ty_generic("Ptr", vec![ty_named("U")])),
            ),
            pointer_method("load", "ptr_load", vec![], vec![], Some(ty_named("T"))),
            pointer_method(
                "load",
                "ptr_load_offset",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_named("T")),
            ),
            pointer_method(
                "store",
                "ptr_store",
                vec![],
                vec![("value", ty_named("T"))],
                None,
            ),
            pointer_method(
                "store",
                "ptr_store_offset",
                vec![],
                vec![("offset", ty_named("Int")), ("value", ty_named("T"))],
                None,
            ),
            pointer_method(
                "plus",
                "ptr_plus",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_generic("Ptr", vec![ty_named("T")])),
            ),
            pointer_method(
                "minus",
                "ptr_minus",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_generic("Ptr", vec![ty_named("T")])),
            ),
        ],
    );
    let Decl::Struct(ptr_decl) = &mut ptr else {
        unreachable!()
    };
    ptr_decl.type_params[0].inline_bound =
        Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));

    let fun_ptr = generic_struct_decl("FunPtr", vec!["F"], vec![("_rawPointer", ty_named("UInt"))]);

    let ref_bound = |mut decl: Decl| {
        let type_params = match &mut decl {
            Decl::Struct(decl) => &mut decl.type_params,
            Decl::Function(decl) => &mut decl.type_params,
            _ => unreachable!("FFI core declarations are structs or functions"),
        };
        type_params[0].inline_bound = Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Ref));
        decl
    };
    let gc_intrinsic =
        |name: &str, intrinsic_name: &str, params: Vec<(&str, TypeRef)>, return_ty: TypeRef| {
            let mut decl =
                intrinsic_generic_fun(name, intrinsic_name, vec!["T"], params, Some(return_ty));
            let Decl::Function(function) = &mut decl else {
                unreachable!()
            };
            function.annotations = vec![marker("Unsafe"), intrinsic(intrinsic_name)];
            ref_bound(decl)
        };
    let top_level = |name: &str,
                     intrinsic_name: &str,
                     params: Vec<(&str, TypeRef)>,
                     return_ty: TypeRef,
                     no_gc: bool,
                     unsafe_: bool| {
        let mut decl =
            intrinsic_generic_fun(name, intrinsic_name, vec!["T"], params, Some(return_ty));
        let Decl::Function(function) = &mut decl else {
            unreachable!()
        };
        function.type_params[0].inline_bound =
            Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));
        function.annotations.clear();
        if no_gc {
            function.annotations.push(marker("NoGC"));
        }
        if unsafe_ {
            function.annotations.push(marker("Unsafe"));
        }
        function.annotations.push(intrinsic(intrinsic_name));
        decl
    };

    let foreign_callback_intrinsic =
        |name: &str, intrinsic_name: &str, params: Vec<(&str, TypeRef)>, return_ty: TypeRef| {
            let mut decl =
                intrinsic_generic_fun(name, intrinsic_name, vec!["F"], params, Some(return_ty));
            let Decl::Function(function) = &mut decl else {
                unreachable!()
            };
            function.annotations = vec![marker("Unsafe"), intrinsic(intrinsic_name)];
            decl
        };

    vec![
        ptr,
        fun_ptr,
        enum_decl(
            "ForeignCallbackMode",
            vec![],
            vec![variant_unit("Reusable"), variant_unit("OneShot")],
        ),
        enum_decl(
            "ForeignCallbackState",
            vec![],
            vec![
                variant_unit("Registered"),
                variant_unit("Active"),
                variant_unit("Completed"),
                variant_unit("Failed"),
            ],
        ),
        generic_struct_decl(
            "ForeignCallback",
            vec!["F"],
            vec![
                ("function", ty_generic("FunPtr", vec![ty_named("F")])),
                ("context", ty_generic("Ptr", vec![ty_named("Unit")])),
            ],
        ),
        foreign_callback_intrinsic(
            "foreignCallback",
            "foreign_callback_register",
            vec![
                ("callback", ty_named("Any")),
                ("contextIndex", ty_named("Int")),
                ("mode", ty_named("ForeignCallbackMode")),
            ],
            ty_generic("ForeignCallback", vec![ty_named("F")]),
        ),
        foreign_callback_intrinsic(
            "retainForeignCallback",
            "foreign_callback_retain",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_generic("ForeignCallback", vec![ty_named("F")]),
        ),
        foreign_callback_intrinsic(
            "releaseForeignCallback",
            "foreign_callback_release",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_named("Unit"),
        ),
        foreign_callback_intrinsic(
            "foreignCallbackState",
            "foreign_callback_state",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_named("ForeignCallbackState"),
        ),
        foreign_callback_intrinsic(
            "foreignCallbackFailure",
            "foreign_callback_failure",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_nullable(ty_named("Throwable")),
        ),
        ref_bound(generic_struct_decl(
            "PinnedPtr",
            vec!["T"],
            vec![("raw", ty_named("UInt"))],
        )),
        ref_bound(generic_struct_decl(
            "GcHandle",
            vec!["T"],
            vec![("raw", ty_named("UInt"))],
        )),
        gc_intrinsic(
            "_pin",
            "gc_pin_raw",
            vec![("v", ty_named("T"))],
            ty_named("UInt"),
        ),
        gc_intrinsic(
            "_unpin",
            "gc_unpin_raw",
            vec![("raw", ty_named("UInt"))],
            ty_named("T"),
        ),
        gc_intrinsic(
            "_getGcHandle",
            "gc_get_handle_raw",
            vec![("v", ty_named("T"))],
            ty_named("UInt"),
        ),
        gc_intrinsic(
            "_releaseGcHandle",
            "gc_release_handle_raw",
            vec![("raw", ty_named("UInt"))],
            ty_named("T"),
        ),
        top_level(
            "addressOf",
            "address_of",
            vec![("value", ty_named("T"))],
            ty_generic("Ptr", vec![ty_named("T")]),
            false,
            true,
        ),
        top_level("sizeOf", "size_of", vec![], ty_named("UInt"), true, false),
        top_level("alignOf", "align_of", vec![], ty_named("UInt"), true, false),
    ]
}

fn gc_api_declarations() -> Vec<Decl> {
    let unsafe_wrapper = |mut decl: Decl| {
        let Decl::Function(function) = &mut decl else {
            unreachable!()
        };
        function.annotations = vec![ast::Annotation {
            name: ident("Unsafe"),
            args: Vec::new(),
            span: sp(),
        }];
        function.type_params[0].inline_bound =
            Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Ref));
        decl
    };
    vec![
        unsafe_wrapper(fun_expr(
            "pin",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(ty_generic("PinnedPtr", vec![ty_named("T")])),
            typed_call(
                "PinnedPtr",
                vec![ty_named("T")],
                vec![call("_pin", vec![var("v")])],
            ),
        )),
        unsafe_wrapper(fun_expr(
            "unpin",
            vec!["T"],
            vec![("p", ty_generic("PinnedPtr", vec![ty_named("T")]))],
            Some(ty_named("T")),
            typed_call("_unpin", vec![ty_named("T")], vec![field(var("p"), "raw")]),
        )),
        unsafe_wrapper(fun_expr(
            "getGcHandle",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(ty_generic("GcHandle", vec![ty_named("T")])),
            typed_call(
                "GcHandle",
                vec![ty_named("T")],
                vec![call("_getGcHandle", vec![var("v")])],
            ),
        )),
        unsafe_wrapper(fun_expr(
            "releaseGcHandle",
            vec!["T"],
            vec![("h", ty_generic("GcHandle", vec![ty_named("T")]))],
            Some(ty_named("T")),
            typed_call(
                "_releaseGcHandle",
                vec![ty_named("T")],
                vec![field(var("h"), "raw")],
            ),
        )),
        intrinsic_fun("gcCollect", "rt_gc_collect", vec![], None),
        intrinsic_fun("gcStats", "rt_gc_stats", vec![], Some(ty_named("UInt"))),
    ]
}

fn exception_core_declarations() -> Vec<Decl> {
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
        subclass("IllegalStateException", "illegal state"),
    ]
}

fn coroutine_core_declarations() -> Vec<Decl> {
    let continuation = generic_interface_decl(
        "Continuation",
        vec![(ast::Variance::In, "T")],
        vec![
            bodyless_method(false, "resume", vec![("value", ty_named("T"))], None),
            bodyless_method(
                false,
                "resumeWithException",
                vec![("exception", ty_named("Throwable"))],
                None,
            ),
        ],
    );
    let task = generic_interface_decl(
        "SuspendTask",
        vec![(ast::Variance::Out, "T")],
        vec![with_suspend(bodyless_method(
            false,
            "run",
            vec![],
            Some(ty_named("T")),
        ))],
    );
    let registration = generic_interface_decl(
        "SuspendRegistration",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(
            false,
            "register",
            vec![(
                "continuation",
                ty_generic("Continuation", vec![ty_named("T")]),
            )],
            None,
        )],
    );
    let start = intrinsic_generic_fun(
        "startCoroutine",
        "coroutine_start",
        vec!["T"],
        vec![
            ("task", ty_generic("SuspendTask", vec![ty_named("T")])),
            (
                "completion",
                ty_generic("Continuation", vec![ty_named("T")]),
            ),
        ],
        None,
    );
    let Decl::Function(mut suspend) = intrinsic_generic_fun(
        "suspendCoroutine",
        "coroutine_suspend",
        vec!["T"],
        vec![(
            "registration",
            ty_generic("SuspendRegistration", vec![ty_named("T")]),
        )],
        Some(ty_named("T")),
    ) else {
        unreachable!("intrinsic_generic_fun always builds a function declaration")
    };
    suspend.is_suspend = true;

    vec![
        continuation,
        task,
        registration,
        start,
        Decl::Function(suspend),
    ]
}

/// Lower a user file together with the minimal `scoop.core`, mirroring
/// the driver's sysroot convention (core files first, user file last).
pub(crate) fn lower_user(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), user]).map(|output| output.export)
}

pub(crate) fn lower_user_output(user: SourceFile) -> Result<hir::Output, Vec<Diagnostic>> {
    lower(&[core_file(), user])
}

// --- M8: exceptions ---

/// `throw expr` (M8).
pub(crate) fn throw_stmt(value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Throw(value),
        span: sp(),
    }
}

/// `try { body } catch... finally...` (M8).
pub(crate) fn try_stmt(
    body: Vec<Statement>,
    catches: Vec<ast::CatchClause>,
    finally_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::Try(ast::Try {
            body: block(body),
            catches,
            finally_body: finally_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

/// `catch (name: T) { body }`.
pub(crate) fn catch_clause(name: &str, ty: TypeRef, body: Vec<Statement>) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span: sp(),
    }
}

/// `catch (name: T) { body }` with an explicit clause span (diagnostic
/// position assertions).
pub(crate) fn catch_clause_at(
    name: &str,
    ty: TypeRef,
    body: Vec<Statement>,
    span: Span,
) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span,
    }
}

/// Lower a user file with the full core exception hierarchy available
/// (M8 tests).
pub(crate) fn lower_user_with_exceptions(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), user]).map(|output| output.export)
}

/// Lower a user file with the core GC facilities available (M9 tests).
pub(crate) fn lower_user_with_gc(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), file(gc_api_declarations()), user]).map(|output| output.export)
}
